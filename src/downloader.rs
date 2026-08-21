//! llama.cpp 下载模块
//!
//! 后台线程完整流程（download_in_background）：
//! 1. GET GitHub latest release API（带 User-Agent）并解析 ReleaseInfo；
//! 2. pick_asset 按变体匹配官方预编译资产（排除 cudart-* 前缀资产）；
//! 3. 流式下载到 base_dir/llama/.partial.<asset.name>（8192 字节/chunk，每 chunk 检查取消并回报进度）；
//! 4. rename partial → 资产文件名，按扩展名解压（.zip → zip crate；.tar.gz → tar + flate2）；
//! 5. find_server_binary 定位 llama-server(.exe)，Linux 下 chmod 0o755；
//! 6. 删除压缩包（best-effort），置 Success(二进制路径)。
//!
//! UI 层：每帧轮询 DownloadHandle::snapshot() 渲染进度条/状态文本；
//! 应用关闭时调用 request_cancel() 协作取消下载线程（下载循环每 chunk 检查取消标志）。
//! 错误消息使用英文字符串（UI 可见部分由 i18n 层拼接前缀）。

use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use serde::Deserialize;

/// GitHub latest release API（ggml-org/llama.cpp）
const LATEST_RELEASE_API: &str = "https://api.github.com/repos/ggml-org/llama.cpp/releases/latest";
/// 请求 User-Agent（GitHub API 必需）
const USER_AGENT: &str = "llama-cpp-launcher";
/// 下载块大小（字节）
const CHUNK_SIZE: usize = 8192;
/// 定位二进制时 BFS 最大深度（相对解压根的子目录深度）
const MAX_SEARCH_DEPTH: usize = 4;

// ======================= 公开类型 =======================

/// 下载阶段（UI 据此展示对应 i18n 文案）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// 正在获取最新版本信息
    FetchingRelease,
    /// 正在下载资产
    Downloading,
    /// 正在解压
    Extracting,
    /// 正在定位 llama-server 二进制
    LocatingServer,
}

/// 下载整体状态（终态携带信息）
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DownloadState {
    /// 空闲（初始 / 已取消）
    Idle,
    /// 进行中（阶段与进度见 DownloadStatus）
    Running,
    /// 成功，携带 llama-server 二进制路径
    Success(String),
    /// 失败，携带英文错误消息（UI 层拼接 i18n 前缀）
    Error(String),
}

/// 下载变体（平台 + 架构 + 推理后端）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadVariant {
    /// Windows x64 CPU
    WinCpu,
    /// Windows x64 CUDA 12.4
    WinCuda124,
    /// Windows x64 CUDA 13.3
    WinCuda133,
    /// Windows x64 ROCm 7.14
    WinRocm714,
    /// Windows x64 Vulkan
    WinVulkan,
    /// Windows arm64 CPU
    WinCpuArm64,
    /// Linux x64 CPU
    LinuxCpu,
    /// Linux x64 Vulkan
    LinuxVulkan,
}

impl DownloadVariant {
    /// 资产名匹配模式（官方资产命名的特征子串）
    /// 例：官方资产 llama-b10549-bin-win-cuda-12.4-x64.zip
    pub fn asset_name(&self) -> &'static str {
        match self {
            DownloadVariant::WinCpu => "bin-win-cpu-x64",
            DownloadVariant::WinCuda124 => "bin-win-cuda-12.4-x64",
            DownloadVariant::WinCuda133 => "bin-win-cuda-13.3-x64",
            DownloadVariant::WinRocm714 => "bin-win-rocm-7.14-x64",
            DownloadVariant::WinVulkan => "bin-win-vulkan-x64",
            DownloadVariant::WinCpuArm64 => "bin-win-cpu-arm64",
            DownloadVariant::LinuxCpu => "bin-ubuntu-x64",
            DownloadVariant::LinuxVulkan => "bin-ubuntu-vulkan-x64",
        }
    }

    /// 资产文件扩展名（用于判断解压方式）
    pub fn extension(&self) -> &'static str {
        match self {
            DownloadVariant::LinuxCpu | DownloadVariant::LinuxVulkan => ".tar.gz",
            _ => ".zip",
        }
    }

    /// 根据配置中的 download_variant 值与当前平台解析出实际下载变体
    ///
    /// - 配置值（与 UI 选项一致）：`cpu` / `cuda124` / `cuda133` / `rocm714` / `vulkan`
    /// - GPU 变体仅在对应平台有效：cuda124/cuda133/rocm714 仅 Windows；vulkan 全平台
    /// - 兼容旧版 `"gpu"`：Windows → CUDA 12.4，Linux → Vulkan
    /// - 兜底：CPU（Linux x64 / Windows arm64 / Windows x64）
    pub fn from_settings_value(value: &str) -> Self {
        let is_linux = cfg!(target_os = "linux");
        match value {
            "cuda124" if !is_linux => DownloadVariant::WinCuda124,
            "cuda133" if !is_linux => DownloadVariant::WinCuda133,
            "rocm714" if !is_linux => DownloadVariant::WinRocm714,
            "vulkan" => {
                if is_linux {
                    DownloadVariant::LinuxVulkan
                } else {
                    DownloadVariant::WinVulkan
                }
            }
            // 兼容旧版 "gpu"
            "gpu" => {
                if is_linux {
                    DownloadVariant::LinuxVulkan
                } else {
                    DownloadVariant::WinCuda124
                }
            }
            // CPU 兜底
            _ => {
                if is_linux {
                    DownloadVariant::LinuxCpu
                } else if cfg!(target_arch = "aarch64") {
                    DownloadVariant::WinCpuArm64
                } else {
                    DownloadVariant::WinCpu
                }
            }
        }
    }
}

/// GitHub release 资产（API 响应的子集）
#[derive(Clone, Debug, Deserialize)]
pub struct Asset {
    /// 资产文件名（如 llama-b10549-bin-win-cpu-x64.zip）
    pub name: String,
    /// 文件大小（字节）
    pub size: u64,
    /// 浏览器下载地址（重定向到实际文件）
    pub browser_download_url: String,
}

/// 共享下载状态：整体状态 + 阶段 + 进度（UI 每帧轮询 snapshot）
#[derive(Clone, Debug)]
pub struct DownloadStatus {
    /// 整体状态
    pub state: DownloadState,
    /// 当前阶段
    pub phase: Phase,
    /// 已下载字节数
    pub done: u64,
    /// 总字节数（仅 Downloading 阶段为 Some）
    pub total: Option<u64>,
}

impl Default for DownloadStatus {
    fn default() -> Self {
        Self {
            state: DownloadState::Idle,
            phase: Phase::FetchingRelease,
            done: 0,
            total: None,
        }
    }
}

/// 下载句柄：由 App 持有，传给 UI 面板
/// - start_download：spawn 后台下载线程（Running 时忽略，防重复点击）
/// - snapshot：UI 每帧轮询当前状态
/// - request_cancel：关窗协作取消（下载循环每 chunk 检查取消标志）
pub struct DownloadHandle {
    /// 协作取消标志
    cancel: Arc<AtomicBool>,
    /// 共享状态
    status: Arc<Mutex<DownloadStatus>>,
    /// worker 线程句柄（线程持有 Arc 克隆，不阻塞应用退出）
    worker: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl Default for DownloadHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl DownloadHandle {
    pub fn new() -> Self {
        Self {
            cancel: Arc::new(AtomicBool::new(false)),
            status: Arc::new(Mutex::new(DownloadStatus::default())),
            worker: Arc::new(Mutex::new(None)),
        }
    }

    /// 若当前非 Running 则 spawn 下载线程；Running 时直接忽略（防重复点击）
    pub fn start_download(&self, base_dir: PathBuf, variant: DownloadVariant) {
        {
            let mut st = self.status.lock().unwrap();
            if matches!(st.state, DownloadState::Running) {
                return;
            }
            st.state = DownloadState::Running;
            st.phase = Phase::FetchingRelease;
            st.done = 0;
            st.total = None;
        }
        // 复位取消标志（允许上次取消后重新下载）
        self.cancel.store(false, Ordering::SeqCst);
        // 清理上一次 worker 句柄（若已结束）
        self.worker.lock().unwrap().take();
        let cancel = Arc::clone(&self.cancel);
        let status = Arc::clone(&self.status);
        let handle = match thread::Builder::new()
            .name("llama-cpp-downloader".to_string())
            .spawn(move || download_in_background(base_dir, variant, cancel, status))
        {
            Ok(h) => h,
            Err(e) => {
                let mut st = self.status.lock().unwrap();
                st.state =
                    DownloadState::Error(format!("failed to spawn downloader thread: {}", e));
                return;
            }
        };
        *self.worker.lock().unwrap() = Some(handle);
    }

    /// UI 每帧调用，返回当前状态克隆
    pub fn snapshot(&self) -> DownloadStatus {
        self.status.lock().unwrap().clone()
    }

    /// Drop/关窗调用，协作取消
    pub fn request_cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    /// 是否正在下载（UI 禁用按钮用）
    pub fn is_busy(&self) -> bool {
        matches!(self.snapshot().state, DownloadState::Running)
    }
}

// ======================= GitHub API 类型 =======================

/// GitHub latest release API 响应（只取需要的字段）
#[derive(Deserialize)]
struct ReleaseInfo {
    tag_name: String,
    assets: Vec<Asset>,
}

/// 请求 GitHub latest release API，返回 release 信息
fn fetch_release() -> Result<ReleaseInfo, String> {
    let body = ureq::get(LATEST_RELEASE_API)
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| format!("fetch latest release failed: {}", e))?
        .into_string()
        .map_err(|e| format!("read release response failed: {}", e))?;
    serde_json::from_str(&body).map_err(|e| format!("parse release info failed: {}", e))
}

/// 获取最新 release 的 tag_name（如 "b10549"），供"检查更新"使用
pub fn fetch_latest_tag() -> Result<String, String> {
    Ok(fetch_release()?.tag_name)
}

// ======================= 下载流程（后台线程） =======================

/// 后台线程入口：执行完整下载流程，结果写入共享状态
pub fn download_in_background(
    base_dir: PathBuf,
    variant: DownloadVariant,
    cancel: Arc<AtomicBool>,
    status: Arc<Mutex<DownloadStatus>>,
) {
    match run_download(&base_dir, variant, &cancel, &status) {
        Ok(path) => {
            let mut st = status.lock().unwrap();
            // 若流程中被取消（解压/定位阶段不检查取消，可能正常完成），按 Idle 处理
            if cancel.load(Ordering::SeqCst) {
                st.state = DownloadState::Idle;
            } else {
                st.state = DownloadState::Success(path);
            }
        }
        Err(message) => {
            let mut st = status.lock().unwrap();
            // 若错误由用户取消引起，回到 Idle 而非显示失败
            if cancel.load(Ordering::SeqCst) {
                st.state = DownloadState::Idle;
            } else {
                st.state = DownloadState::Error(message);
            }
        }
    }
}

/// 完整下载流程；成功返回 llama-server 二进制路径（字符串）
fn run_download(
    base_dir: &Path,
    variant: DownloadVariant,
    cancel: &AtomicBool,
    status: &Arc<Mutex<DownloadStatus>>,
) -> Result<String, String> {
    // 1) 获取最新版本信息
    set_running(status, Phase::FetchingRelease, 0, None);
    let release = fetch_release()?;

    // 2) 按变体匹配资产
    let asset = pick_asset(&release.assets, variant).ok_or_else(|| {
        format!(
            "no matching asset for pattern '{}' in release {}",
            variant.asset_name(),
            release.tag_name
        )
    })?;

    // 3) 流式下载（partial 文件 + rename 原子落盘）
    set_running(status, Phase::Downloading, 0, Some(asset.size));
    let llama_dir = base_dir.join("llama");
    fs::create_dir_all(&llama_dir).map_err(|e| format!("create llama dir failed: {}", e))?;
    let partial = llama_dir.join(format!(".partial.{}", asset.name));
    match download_to_file(&asset.browser_download_url, &partial, cancel, status) {
        Ok(()) => {}
        Err(DlError::Cancelled) => {
            // 取消：清理 partial 文件
            let _ = fs::remove_file(&partial);
            return Err("download cancelled".to_string());
        }
        Err(DlError::Failed(e)) => return Err(format!("download failed: {}", e)),
    }
    // 下载完成后再次检查取消（防最后一 chunk 后取消）
    if cancel.load(Ordering::SeqCst) {
        let _ = fs::remove_file(&partial);
        return Err("download cancelled".to_string());
    }
    let archive_path = llama_dir.join(&asset.name);
    fs::rename(&partial, &archive_path)
        .map_err(|e| format!("rename partial file failed: {}", e))?;

    // 4) 按扩展名解压到 llama_dir
    set_running(status, Phase::Extracting, 0, None);
    if asset.name.ends_with(".zip") {
        extract_zip(&archive_path, &llama_dir).map_err(|e| format!("extract zip failed: {}", e))?;
    } else if asset.name.ends_with(".tar.gz") {
        extract_tar_gz(&archive_path, &llama_dir)
            .map_err(|e| format!("extract tar.gz failed: {}", e))?;
    }

    // 5) 定位 llama-server 二进制
    set_running(status, Phase::LocatingServer, 0, None);
    let windows = cfg!(target_os = "windows");
    let stem = asset_stem(&asset.name);
    let binary = find_server_binary(&llama_dir, &stem, windows)
        .ok_or_else(|| "llama-server binary not found in extracted files".to_string())?;

    // 6) Linux：确保 bin 目录内文件可执行（best-effort）
    if !windows {
        if let Some(bin_dir) = binary.parent() {
            chmod_all(bin_dir);
        }
    }

    // 7) 删除压缩包（best-effort，失败忽略）
    let _ = fs::remove_file(&archive_path);

    Ok(binary.to_string_lossy().to_string())
}

/// 下载错误：用户取消 vs 普通失败
enum DlError {
    Cancelled,
    Failed(String),
}

/// 流式下载到文件（8192 字节/chunk；每 chunk 更新进度 + 检查取消）
fn download_to_file(
    url: &str,
    out: &Path,
    cancel: &AtomicBool,
    status: &Arc<Mutex<DownloadStatus>>,
) -> Result<(), DlError> {
    let response = ureq::get(url)
        .set("User-Agent", USER_AGENT)
        .call()
        .map_err(|e| DlError::Failed(e.to_string()))?;
    let file = File::create(out).map_err(|e| DlError::Failed(e.to_string()))?;
    let mut writer = BufWriter::new(file);
    let mut reader = response.into_reader();
    let mut buf = vec![0u8; CHUNK_SIZE];
    let mut done: u64 = 0;
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| DlError::Failed(e.to_string()))?;
        if n == 0 {
            break;
        }
        writer
            .write_all(&buf[..n])
            .map_err(|e| DlError::Failed(e.to_string()))?;
        done += n as u64;
        // 回报进度
        {
            let mut st = status.lock().unwrap();
            st.done = done;
        }
        // 协作取消检查
        if cancel.load(Ordering::SeqCst) {
            return Err(DlError::Cancelled);
        }
    }
    writer.flush().map_err(|e| DlError::Failed(e.to_string()))?;
    Ok(())
}

/// 解压 .zip（zip 2.x 的 ZipArchive::extract 自带 zip-slip 防护）
fn extract_zip(archive: &Path, dest: &Path) -> Result<(), String> {
    let file = File::open(archive).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(BufReader::new(file)).map_err(|e| e.to_string())?;
    zip.extract(dest).map_err(|e| e.to_string())?;
    Ok(())
}

/// 解压 .tar.gz（flate2 gzip 解码 + tar 解包）
fn extract_tar_gz(archive: &Path, dest: &Path) -> Result<(), String> {
    let file = File::open(archive).map_err(|e| e.to_string())?;
    let decoder = flate2::bufread::GzDecoder::new(BufReader::new(file));
    let mut tar = tar::Archive::new(decoder);
    tar.unpack(dest).map_err(|e| e.to_string())?;
    Ok(())
}

/// Linux：对目录内所有文件 chmod 0o755（best-effort，失败忽略）
#[cfg(unix)]
fn chmod_all(dir: &Path) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o755));
            }
        }
    }
}

/// 非 Linux 平台：无操作
#[cfg(not(unix))]
fn chmod_all(_dir: &Path) {}

/// 置状态为 Running（携带阶段与进度）
fn set_running(status: &Arc<Mutex<DownloadStatus>>, phase: Phase, done: u64, total: Option<u64>) {
    let mut st = status.lock().unwrap();
    st.state = DownloadState::Running;
    st.phase = phase;
    st.done = done;
    st.total = total;
}

// ======================= 纯函数（平台/架构全走参数，可跨平台单测） =======================

/// 按变体匹配资产（取第一个匹配）：
/// - 排除 name 以 "cudart-" 开头的资产（CUDA runtime，非启动器所需）；
/// - name 含 variant.asset_name() 特征子串，且以 variant.extension() 结尾
pub fn pick_asset(assets: &[Asset], variant: DownloadVariant) -> Option<&Asset> {
    let pattern = variant.asset_name();
    let ext = variant.extension();
    assets.iter().find(|a| {
        !a.name.starts_with("cudart-") && a.name.contains(pattern) && a.name.ends_with(ext)
    })
}

/// 定位解压后的 llama-server 二进制：
/// 1) 优先官方资产标准路径：<extract_root>/<asset_stem>/build/bin/llama-server(.exe)；
/// 2) 兜底 BFS（限深 4），目录排序：名称含 "llama"（忽略大小写）者优先，再按字典序 —— 保证确定性；
///    read_dir 错误静默跳过
pub fn find_server_binary(extract_root: &Path, asset_stem: &str, windows: bool) -> Option<PathBuf> {
    let exe_name = if windows {
        "llama-server.exe"
    } else {
        "llama-server"
    };
    // 1) 官方资产标准路径
    let direct = extract_root
        .join(asset_stem)
        .join("build")
        .join("bin")
        .join(exe_name);
    if direct.is_file() {
        return Some(direct);
    }
    // 2) BFS 兜底（版本更新可能改变目录结构）
    bfs_find_binary(extract_root, exe_name, MAX_SEARCH_DEPTH)
}

/// BFS 在 root 下查找名为 exe_filename 的文件（限深：相对 root 的子目录深度）
fn bfs_find_binary(root: &Path, exe_filename: &str, max_depth: usize) -> Option<PathBuf> {
    // 先检查 root 本身
    if let Some(p) = find_in_dir(root, exe_filename) {
        return Some(p);
    }
    let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::new();
    push_subdirs(&mut queue, root, 1);
    while let Some((dir, depth)) = queue.pop_front() {
        if let Some(p) = find_in_dir(&dir, exe_filename) {
            return Some(p);
        }
        if depth < max_depth {
            push_subdirs(&mut queue, &dir, depth + 1);
        }
    }
    None
}

/// 将 dir 的子目录按确定性顺序入队（含 "llama" 关键词者优先，再字典序）
fn push_subdirs(queue: &mut VecDeque<(PathBuf, usize)>, dir: &Path, depth: usize) {
    let Ok(rd) = fs::read_dir(dir) else {
        return; // read_dir 错误静默跳过
    };
    let mut dirs: Vec<PathBuf> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort_by_key(|d| sort_key(d));
    for d in dirs {
        queue.push_back((d, depth));
    }
}

/// 目录排序键：名称含 "llama"（忽略大小写）者优先，再按忽略大小写字典序
fn sort_key(path: &Path) -> (bool, String) {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    // false < true → 含 "llama" 的目录排在前面
    (!name.contains("llama"), name)
}

/// 在 dir 直接下层查找名为 exe_filename 的文件（命中即返回）
fn find_in_dir(dir: &Path, exe_filename: &str) -> Option<PathBuf> {
    let Ok(rd) = fs::read_dir(dir) else {
        return None;
    };
    for entry in rd.flatten() {
        let p = entry.path();
        if p.is_file() && p.file_name().map(|n| n == exe_filename).unwrap_or(false) {
            return Some(p);
        }
    }
    None
}

/// 去掉资产文件名的扩展名，得到解压后的目录名（如 llama-b10549-bin-win-cpu-x64）
fn asset_stem(name: &str) -> String {
    name.strip_suffix(".tar.gz")
        .or_else(|| name.strip_suffix(".zip"))
        .unwrap_or(name)
        .to_string()
}

// ======================= 单元测试 =======================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// 构造测试用 Asset
    fn asset(name: &str) -> Asset {
        Asset {
            name: name.to_string(),
            size: 1,
            browser_download_url: format!(
                "https://github.com/ggml-org/llama.cpp/releases/download/test/{}",
                name
            ),
        }
    }

    #[test]
    fn pick_asset_win_cpu_hit() {
        let assets = vec![
            asset("llama-b10549-bin-win-cuda-12.4-x64.zip"),
            asset("llama-b10549-bin-win-cpu-x64.zip"),
        ];
        let picked = pick_asset(&assets, DownloadVariant::WinCpu).expect("应匹配 WinCpu 资产");
        assert_eq!(picked.name, "llama-b10549-bin-win-cpu-x64.zip");
    }

    #[test]
    fn pick_asset_win_cuda_versions() {
        let assets = vec![
            asset("llama-b10549-bin-win-cuda-12.4-x64.zip"),
            asset("llama-b10549-bin-win-cuda-13.3-x64.zip"),
        ];
        let p124 = pick_asset(&assets, DownloadVariant::WinCuda124).expect("应匹配 CUDA 12.4 资产");
        assert_eq!(p124.name, "llama-b10549-bin-win-cuda-12.4-x64.zip");
        let p133 = pick_asset(&assets, DownloadVariant::WinCuda133).expect("应匹配 CUDA 13.3 资产");
        assert_eq!(p133.name, "llama-b10549-bin-win-cuda-13.3-x64.zip");
    }

    #[test]
    fn pick_asset_win_cpu_arm64_hit() {
        let assets = vec![
            asset("llama-b10549-bin-win-cpu-x64.zip"),
            asset("llama-b10549-bin-win-cpu-arm64.zip"),
        ];
        let picked = pick_asset(&assets, DownloadVariant::WinCpuArm64).expect("应匹配 arm64 资产");
        assert_eq!(picked.name, "llama-b10549-bin-win-cpu-arm64.zip");
        // x64 变体不应命中 arm64 资产
        assert!(pick_asset(&[assets[1].clone()], DownloadVariant::WinCpu).is_none());
    }

    #[test]
    fn pick_asset_excludes_cudart() {
        // 仅 cudart-* 资产 → None
        let only_cudart = vec![asset("cudart-llama-b10549-bin-win-cuda-12.4-x64.zip")];
        assert!(pick_asset(&only_cudart, DownloadVariant::WinCuda124).is_none());
        // cudart 与官方资产并存 → 选官方资产
        let mixed = vec![
            asset("cudart-llama-b10549-bin-win-cuda-12.4-x64.zip"),
            asset("llama-b10549-bin-win-cuda-12.4-x64.zip"),
        ];
        let picked =
            pick_asset(&mixed, DownloadVariant::WinCuda124).expect("应跳过 cudart 选中官方资产");
        assert_eq!(picked.name, "llama-b10549-bin-win-cuda-12.4-x64.zip");
    }

    #[test]
    fn pick_asset_linux_cpu_vs_vulkan() {
        let assets = vec![
            asset("llama-b10549-bin-ubuntu-vulkan-x64.tar.gz"),
            asset("llama-b10549-bin-ubuntu-x64.tar.gz"),
        ];
        let cpu = pick_asset(&assets, DownloadVariant::LinuxCpu).expect("应匹配 Linux CPU 资产");
        assert_eq!(cpu.name, "llama-b10549-bin-ubuntu-x64.tar.gz");
        let vulkan = pick_asset(&assets, DownloadVariant::LinuxVulkan).expect("应匹配 Vulkan 资产");
        assert_eq!(vulkan.name, "llama-b10549-bin-ubuntu-vulkan-x64.tar.gz");
    }

    #[test]
    fn pick_asset_win_rocm_and_vulkan() {
        let assets = vec![
            asset("llama-b10549-bin-win-rocm-7.14-x64.zip"),
            asset("llama-b10549-bin-win-vulkan-x64.zip"),
            asset("llama-b10549-bin-win-cuda-13.3-x64.zip"),
        ];
        let rocm = pick_asset(&assets, DownloadVariant::WinRocm714).expect("应匹配 ROCm 7.14 资产");
        assert_eq!(rocm.name, "llama-b10549-bin-win-rocm-7.14-x64.zip");
        let vulkan =
            pick_asset(&assets, DownloadVariant::WinVulkan).expect("应匹配 Win Vulkan 资产");
        assert_eq!(vulkan.name, "llama-b10549-bin-win-vulkan-x64.zip");
    }

    #[test]
    fn from_settings_value_gpu_variants() {
        // 平台相关的断言按当前编译目标条件化，保证跨平台可编译
        if cfg!(target_os = "linux") {
            assert_eq!(
                DownloadVariant::from_settings_value("cpu"),
                DownloadVariant::LinuxCpu
            );
            assert_eq!(
                DownloadVariant::from_settings_value("vulkan"),
                DownloadVariant::LinuxVulkan
            );
            // 兼容旧版 "gpu"
            assert_eq!(
                DownloadVariant::from_settings_value("gpu"),
                DownloadVariant::LinuxVulkan
            );
            // Linux 无 CUDA/ROCm 资产，回落到 CPU
            assert_eq!(
                DownloadVariant::from_settings_value("cuda133"),
                DownloadVariant::LinuxCpu
            );
        } else {
            let expected_cpu = if cfg!(target_arch = "aarch64") {
                DownloadVariant::WinCpuArm64
            } else {
                DownloadVariant::WinCpu
            };
            assert_eq!(DownloadVariant::from_settings_value("cpu"), expected_cpu);
            assert_eq!(
                DownloadVariant::from_settings_value("cuda124"),
                DownloadVariant::WinCuda124
            );
            assert_eq!(
                DownloadVariant::from_settings_value("cuda133"),
                DownloadVariant::WinCuda133
            );
            assert_eq!(
                DownloadVariant::from_settings_value("rocm714"),
                DownloadVariant::WinRocm714
            );
            assert_eq!(
                DownloadVariant::from_settings_value("vulkan"),
                DownloadVariant::WinVulkan
            );
            // 兼容旧版 "gpu"
            assert_eq!(
                DownloadVariant::from_settings_value("gpu"),
                DownloadVariant::WinCuda124
            );
        }
    }

    #[test]
    fn pick_asset_no_match_returns_none() {
        let assets = vec![asset("llama-b10549-bin-win-cpu-x64.zip")];
        assert!(pick_asset(&assets, DownloadVariant::LinuxVulkan).is_none());
        assert!(pick_asset(&[], DownloadVariant::WinCpu).is_none());
    }

    #[test]
    fn find_server_binary_standard_deep_path() {
        let tmp = tempfile::tempdir().expect("创建临时目录失败");
        let root = tmp.path().join("llama");
        let bin_dir = root
            .join("llama-b10549-bin-win-cpu-x64")
            .join("build")
            .join("bin");
        fs::create_dir_all(&bin_dir).expect("创建 bin 目录失败");
        let exe = bin_dir.join("llama-server.exe");
        fs::write(&exe, b"").expect("写入假 exe 失败");

        let found = find_server_binary(&root, "llama-b10549-bin-win-cpu-x64", true)
            .expect("应在标准深路径找到 llama-server.exe");
        assert_eq!(found, exe);
    }

    #[test]
    fn find_server_binary_empty_dir_returns_none() {
        let tmp = tempfile::tempdir().expect("创建临时目录失败");
        let root = tmp.path().join("llama");
        fs::create_dir_all(&root).expect("创建根目录失败");

        assert!(find_server_binary(&root, "llama-b10549-bin-win-cpu-x64", true).is_none());
    }
}
