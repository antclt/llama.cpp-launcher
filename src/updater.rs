//! 软件自更新模块（单文件 exe 绿色版）
//!
//! 流程：
//! 1. check()：GET GitHub latest release API（yihuishou/llama.cpp-launcher），
//!    比对 tag_name 与当前编译版本（CARGO_PKG_VERSION）；
//! 2. 有新版本时 UI 切换为「安装更新」，install() 后台线程流式下载新 exe
//!    （写入 exe 同目录 update/ 子目录，带进度 + 取消）；
//! 3. 下载完成 → replace_and_restart()：以 cmd /d /c 启动分离脚本
//!    （脚本参数经进程 API 传递，中英文路径均安全）：
//!      timeout 3 秒（等主进程退出）
//!      → move 当前 exe → .old
//!      → move 新 exe → 正式名
//!      → del .old
//!      → start 新 exe（自动重启）
//! 4. 主进程随即退出窗口。
//!
//! UI 层：每帧轮询 UpdaterHandle::snapshot() 渲染按钮状态/进度；
//! 应用关闭时无需额外取消（下载线程持有 Arc 克隆，不阻塞退出）。

use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use serde::Deserialize;

/// 上游仓库 latest release API（直连，官方）
const API_OFFICIAL: &str =
    "https://api.github.com/repos/yihuishou/llama.cpp-launcher/releases/latest";
/// API 镜像基址（gh-proxy 前缀，官方超时/失败时自动回退）
const API_MIRROR: &str =
    "https://gh-proxy.com/https://api.github.com/repos/yihuishou/llama.cpp-launcher/releases/latest";
/// 资产下载源（镜像前缀；官方 URL 直接使用 asset.browser_download_url）
const DOWNLOAD_MIRROR: &str = "https://gh-proxy.com/https://github.com";
/// 官方 GitHub API 超时（秒）
const OFFICIAL_TIMEOUT_SECS: u64 = 16;
/// gh-proxy 镜像超时（秒）
const MIRROR_TIMEOUT_SECS: u64 = 32;
/// 全部源都失败时 UI 展示的固定错误消息（网络层；区别于业务错误）
pub const ERR_NETWORK: &str = "network-error";
/// 请求 User-Agent（GitHub API 必需）
const USER_AGENT: &str = "llama-cpp-launcher";
/// 下载块大小（字节）
const CHUNK_SIZE: usize = 8192;

/// 获取当前 CPU 架构标识（用于匹配 Linux 资产）
fn get_arch() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else {
        // 兜底：未知架构返回空字符串，后续匹配会失败
        ""
    }
}

/// 带超时与 User-Agent 的共享 Agent
fn agent(timeout_secs: u64) -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()
}

/// 官方 + 镜像依次尝试，带不同超时；返回 Response body 字符串
fn fetch_with_fallback(official_url: &str, mirror_url: &str) -> Result<String, String> {
    let official_agent = agent(OFFICIAL_TIMEOUT_SECS);
    if let Ok(body) = fetch_body(&official_agent, official_url) {
        return Ok(body);
    }
    let mirror_agent = agent(MIRROR_TIMEOUT_SECS);
    if let Ok(body) = fetch_body(&mirror_agent, mirror_url) {
        return Ok(body);
    }
    Err(ERR_NETWORK.to_string())
}

/// 单次 GET + 读取 body
fn fetch_body(ag: &ureq::Agent, url: &str) -> Result<String, String> {
    let response = ag
        .get(url)
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| e.to_string())?;
    response.into_string().map_err(|e| e.to_string())
}

// ======================= 公开类型 =======================

/// 更新流程状态（UI 据此渲染按钮与提示）
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateState {
    /// 空闲（初始 / 已取消）
    Idle,
    /// 正在查询 GitHub API
    Checking,
    /// 已是最新版本（UI 显示"已是最新"）
    UpToDate,
    /// 发现新版本，携带版本号（如 "0.1.20"）
    Available(String),
    /// 正在下载更新（进度见 UpdateStatus::done / total）
    Downloading,
    /// 下载完成，正在启动自替换（即将重启）
    Installing,
    /// 失败，携带英文错误消息（UI 层拼接 i18n 前缀）
    Error(String),
}

/// 更新状态快照（UI 每帧轮询）
#[derive(Clone, Debug)]
pub struct UpdateStatus {
    pub state: UpdateState,
    /// 已下载字节
    pub done: u64,
    /// 总字节（未知为 0）
    pub total: u64,
}

impl Default for UpdateStatus {
    fn default() -> Self {
        Self {
            state: UpdateState::Idle,
            done: 0,
            total: 0,
        }
    }
}

/// 后台更新控制句柄（模式对齐 downloader::DownloadHandle）
pub struct UpdaterHandle {
    /// 协作取消标志
    cancel: Arc<AtomicBool>,
    /// 共享状态
    status: Arc<Mutex<UpdateStatus>>,
    /// worker 线程句柄（线程持有 Arc 克隆，不阻塞应用退出）
    worker: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl Default for UpdaterHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl UpdaterHandle {
    pub fn new() -> Self {
        Self {
            cancel: Arc::new(AtomicBool::new(false)),
            status: Arc::new(Mutex::new(UpdateStatus::default())),
            worker: Arc::new(Mutex::new(None)),
        }
    }

    /// 检查更新：仅 Idle / UpToDate / Error 时启动检查线程（防重复）
    pub fn check(&self) {
        let busy = matches!(
            self.status.lock().unwrap().state,
            UpdateState::Checking | UpdateState::Downloading | UpdateState::Installing
        );
        if busy {
            return;
        }
        self.status.lock().unwrap().state = UpdateState::Checking;
        self.spawn_worker(WorkerKind::Check);
    }

    /// 开始下载并安装：仅 Available 时启动（Downloading 时忽略）
    pub fn install(&self) {
        let ready = matches!(self.status.lock().unwrap().state, UpdateState::Available(_));
        if !ready {
            return;
        }
        self.cancel.store(false, Ordering::SeqCst);
        self.spawn_worker(WorkerKind::Download);
    }

    /// 返回当前状态快照
    pub fn snapshot(&self) -> UpdateStatus {
        self.status.lock().unwrap().clone()
    }

    /// 是否正在下载（UI 禁用按钮用）
    pub fn is_busy(&self) -> bool {
        matches!(
            self.snapshot().state,
            UpdateState::Checking | UpdateState::Downloading | UpdateState::Installing
        )
    }

    /// Drop/关窗调用，协作取消下载
    pub fn request_cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    fn spawn_worker(&self, kind: WorkerKind) {
        let cancel = Arc::clone(&self.cancel);
        let status = Arc::clone(&self.status);
        let handle = thread::spawn(move || match kind {
            WorkerKind::Check => worker_check(status),
            WorkerKind::Download => worker_download(cancel, status),
        });
        *self.worker.lock().unwrap() = Some(handle);
    }
}

enum WorkerKind {
    Check,
    Download,
}

// ======================= GitHub API 类型 =======================

/// GitHub latest release API 响应（只取需要的字段）
#[derive(Deserialize)]
struct ReleaseInfo {
    tag_name: String,
    assets: Vec<ReleaseAsset>,
}

#[derive(Deserialize)]
struct ReleaseAsset {
    name: String,
    size: u64,
    browser_download_url: String,
}

/// 请求 GitHub latest release API：官方直连失败（超时/网络错误）时自动尝试镜像。
/// 全部源失败返回 ERR_NETWORK（固定标记，UI 展示"获取失败：网络错误"）。
fn fetch_release() -> Result<ReleaseInfo, String> {
    let body = fetch_with_fallback(API_OFFICIAL, API_MIRROR)?;
    serde_json::from_str::<ReleaseInfo>(&body).map_err(|e| e.to_string())
}

/// 匹配当前平台的 launcher 资产
/// - Windows: llama_cpp_launcher_v*.exe
/// - Linux: llama_cpp_launcher_v*_linux_{arch}.tar.gz
fn pick_launcher_asset(assets: &[ReleaseAsset]) -> Option<&ReleaseAsset> {
    let is_linux = cfg!(target_os = "linux");
    let arch = get_arch();

    assets.iter().find(|a| {
        if is_linux {
            // Linux: 匹配 llama_cpp_launcher_v*_linux_{arch}.tar.gz
            a.name.starts_with("llama_cpp_launcher_v")
                && a.name.contains("_linux_")
                && a.name.contains(arch)
                && a.name.ends_with(".tar.gz")
        } else {
            // Windows: 匹配 llama_cpp_launcher_v*.exe
            a.name.starts_with("llama_cpp_launcher_v") && a.name.ends_with(".exe")
        }
    })
}

/// 版本号比较："v0.1.20" vs "0.1.19" → 新版本存在（按点分数字比较）
fn is_newer(tag: &str, current: &str) -> bool {
    fn nums(s: &str) -> Vec<u64> {
        s.trim_start_matches('v')
            .split('.')
            .filter_map(|p| p.parse::<u64>().ok())
            .collect()
    }
    let t = nums(tag);
    let c = nums(current);
    for (a, b) in t.iter().zip(c.iter()) {
        if a != b {
            return a > b;
        }
    }
    t.len() > c.len()
}

// ======================= 检查线程 =======================

fn worker_check(status: Arc<Mutex<UpdateStatus>>) {
    let current = env!("CARGO_PKG_VERSION");
    match fetch_release() {
        Ok(release) => {
            let mut st = status.lock().unwrap();
            if is_newer(&release.tag_name, current) {
                st.state = UpdateState::Available(trim_v(&release.tag_name));
            } else {
                st.state = UpdateState::UpToDate;
            }
        }
        Err(message) => {
            let mut st = status.lock().unwrap();
            st.state = UpdateState::Error(message);
        }
    }
}

fn trim_v(tag: &str) -> String {
    tag.trim_start_matches('v').to_string()
}

// ======================= 下载线程 =======================

fn worker_download(cancel: Arc<AtomicBool>, status: Arc<Mutex<UpdateStatus>>) {
    let result = run_download(&cancel, &status);
    match result {
        Ok(new_exe) => {
            let mut st = status.lock().unwrap();
            if cancel.load(Ordering::SeqCst) {
                st.state = UpdateState::Idle;
            } else {
                // 下载完成：启动自替换（拉起的命令行已捕获新 exe 路径）
                st.state = UpdateState::Installing;
                st.total = st.done;
                drop(st);
                replace_and_restart(&new_exe);
            }
        }
        Err(message) => {
            let mut st = status.lock().unwrap();
            if cancel.load(Ordering::SeqCst) {
                st.state = UpdateState::Idle;
            } else {
                st.state = UpdateState::Error(message);
            }
        }
    }
}

/// 下载新版本到 exe 同目录 update/ 子目录；成功返回新二进制完整路径。
/// Windows 下载 .exe；Linux 下载 .tar.gz 并解压得到二进制。
/// 官方下载源失败（超时/网络错误）时自动尝试镜像；全部失败返回 ERR_NETWORK。
fn run_download(cancel: &AtomicBool, status: &Arc<Mutex<UpdateStatus>>) -> Result<PathBuf, String> {
    // 1) 获取 release 信息与资产
    let release = fetch_release()?;
    let asset = pick_launcher_asset(&release.assets)
        .ok_or_else(|| format!("no launcher asset in release {}", release.tag_name))?;

    // 2) 目标目录：exe 同目录/update
    let exe_path = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_dir = exe_path.parent().ok_or("no exe parent dir")?.to_path_buf();
    let update_dir = exe_dir.join("update");
    fs::create_dir_all(&update_dir).map_err(|e| format!("create update dir failed: {}", e))?;

    let is_linux = cfg!(target_os = "linux");
    let new_binary = if is_linux {
        update_dir.join("llama_cpp_launcher_new")
    } else {
        update_dir.join("llama_cpp_launcher_new.exe")
    };

    // 3) 流式下载（partial + rename 原子落盘）；官方/镜像两源依次尝试
    {
        let mut st = status.lock().unwrap();
        st.state = UpdateState::Downloading;
        st.done = 0;
        st.total = asset.size;
    }
    let partial_ext = if is_linux { ".tar.gz" } else { ".exe" };
    let partial = update_dir.join(format!(".partial.new{}", partial_ext));

    // asset.browser_download_url 形如 https://github.com/.../releases/download/...
    // 官方源直接使用；镜像源去掉 github.com 前缀后接在 DOWNLOAD_MIRROR 之后
    // （DOWNLOAD_MIRROR 以 gh-proxy.com/https://github.com 结尾，不可重复拼接前缀）
    let official_url = asset.browser_download_url.clone();
    let mirror_url = match official_url.strip_prefix("https://github.com") {
        Some(rest) => format!("{}{}", DOWNLOAD_MIRROR, rest),
        None => official_url.clone(),
    };

    for (i, url) in [official_url, mirror_url].iter().enumerate() {
        if cancel.load(Ordering::SeqCst) {
            return Err("download cancelled".to_string());
        }
        let _ = fs::remove_file(&partial); // 上一源失败的残留
                                           // i=0 官方源 16s，i=1 镜像源 32s
        let timeout = if i == 0 {
            OFFICIAL_TIMEOUT_SECS
        } else {
            MIRROR_TIMEOUT_SECS
        };
        let ag = agent(timeout);
        let response = match ag.get(url).set("User-Agent", USER_AGENT).call() {
            Ok(r) => r,
            Err(_) => continue, // 该源失败，尝试下一个
        };
        // 流式写入
        let result = (|| -> Result<u64, String> {
            let file =
                File::create(&partial).map_err(|e| format!("create temp file failed: {}", e))?;
            let mut writer = BufWriter::new(file);
            let mut reader = response.into_reader();
            let mut buf = vec![0u8; CHUNK_SIZE];
            let mut done: u64 = 0;
            loop {
                let n = reader
                    .read(&mut buf)
                    .map_err(|e| format!("download failed: {}", e))?;
                if n == 0 {
                    break;
                }
                writer
                    .write_all(&buf[..n])
                    .map_err(|e| format!("write failed: {}", e))?;
                done += n as u64;
                status.lock().unwrap().done = done;
                if cancel.load(Ordering::SeqCst) {
                    return Err("download cancelled".to_string());
                }
            }
            writer.flush().map_err(|e| e.to_string())?;
            Ok(done)
        })();
        let done = match result {
            Ok(d) => d,
            Err(msg) => {
                if msg == "download cancelled" {
                    return Err(msg);
                }
                continue; // 该源失败，尝试下一个
            }
        };
        if cancel.load(Ordering::SeqCst) {
            let _ = fs::remove_file(&partial);
            return Err("download cancelled".to_string());
        }
        // 下载完整性校验：实际字节应与资产声明一致
        if done != asset.size {
            continue; // 该源数据不完整，尝试下一个
        }
        fs::rename(&partial, &new_binary).map_err(|e| format!("rename temp file failed: {}", e))?;

        // Linux: 需要从 .tar.gz 解压出二进制文件
        if is_linux {
            return extract_linux_binary(&new_binary, &update_dir);
        }
        return Ok(new_binary);
    }
    let _ = fs::remove_file(&partial);
    Err(ERR_NETWORK.to_string()) // 官方 + 镜像均失败
}

/// 从 .tar.gz 解压出 launcher 二进制文件
fn extract_linux_binary(archive: &Path, dest_dir: &Path) -> Result<PathBuf, String> {
    let file = File::open(archive).map_err(|e| format!("open archive failed: {}", e))?;
    let decoder = flate2::bufread::GzDecoder::new(BufReader::new(file));
    let mut tar = tar::Archive::new(decoder);

    // 解压到目标目录
    tar.unpack(dest_dir)
        .map_err(|e| format!("extract tar.gz failed: {}", e))?;

    // 查找解压出的二进制文件（llama_cpp_launcher 或 llama_cpp_launcher.exe）
    let entries = fs::read_dir(dest_dir).map_err(|e| format!("read dir failed: {}", e))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            // 匹配 launcher 二进制（排除压缩包本身）
            if (name.starts_with("llama_cpp_launcher") || name == "llama-cpp-launcher")
                && !name.ends_with(".tar.gz")
                && !name.ends_with(".partial")
            {
                // 确保文件可执行
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o755));
                }
                return Ok(path);
            }
        }
    }

    Err("launcher binary not found in archive".to_string())
}

// ======================= 自替换 =======================

/// 启动独立脚本完成替换并重启：
///   等待主进程退出 → 当前二进制改名为 .old → 新二进制移入正式名 → 删除 .old → 启动新二进制。
/// 脚本通过进程参数传递，中英文路径均安全；主进程随后自行退出。
fn replace_and_restart(new_binary: &Path) {
    let Ok(exe_path) = std::env::current_exe() else {
        return;
    };
    let old_binary = exe_path.with_extension("old");

    #[cfg(target_os = "windows")]
    {
        // Windows: 使用 cmd /d /c 启动脚本
        // timeout /t 8 /nobreak >nul 等待8秒，确保主进程完全退出
        let update_dir = exe_path
            .parent()
            .map(|p| p.join("update"))
            .unwrap_or_default();
        let script = format!(
            "timeout /t 8 /nobreak >nul & \
             move /Y \"{}\" \"{}\" & \
             move /Y \"{}\" \"{}\" & \
             del /Q \"{}\" & \
             rmdir /Q /S \"{}\" & \
             start \"\" \"{}\"",
            exe_path.display(),
            old_binary.display(),
            new_binary.display(),
            exe_path.display(),
            old_binary.display(),
            update_dir.display(),
            exe_path.display(),
        );
        let mut cmd = std::process::Command::new("cmd");
        cmd.args(["/d", "/c", &script]);
        {
            use std::os::windows::process::CommandExt;
            // CREATE_NO_WINDOW | DETACHED_PROCESS
            cmd.creation_flags(0x0800_0000 | 0x0000_0008);
        }
        let _ = cmd.spawn();
    }

    #[cfg(not(target_os = "windows"))]
    {
        // Linux/macOS: sh 后台执行，sleep 等主进程退出后替换并重启
        let update_dir = exe_path
            .parent()
            .map(|p| p.join("update"))
            .unwrap_or_default();
        let script = format!(
            "(sleep 8; mv \"{}\" \"{}\"; mv \"{}\" \"{}\"; rm -f \"{}\"; rm -rf \"{}\"; chmod +x \"{}\"; \"{}\" &) >/dev/null 2>&1 &",
            exe_path.display(),
            old_binary.display(),
            new_binary.display(),
            exe_path.display(),
            old_binary.display(),
            update_dir.display(),
            exe_path.display(),
            exe_path.display(),
        );
        let _ = std::process::Command::new("sh")
            .args(["-c", &script])
            .spawn();
    }
}
