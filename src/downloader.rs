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

/// 请求 User-Agent（GitHub API 必需）
const USER_AGENT: &str = "llama-cpp-launcher";
/// 下载块大小（字节）
const CHUNK_SIZE: usize = 8192;
/// 定位二进制时 BFS 最大深度（相对解压根的子目录深度）
const MAX_SEARCH_DEPTH: usize = 4;
/// 单次网络请求超时（秒）；超时后自动尝试下一个源
const NETWORK_TIMEOUT_SECS: u64 = 8;
/// 全部源都失败时 UI 展示的固定错误消息（网络层）
pub const ERR_NETWORK: &str = "network-error";
/// API 镜像基址（gh-proxy 前缀，官方超时/失败时自动回退）
const API_MIRROR_BASE: &str = "https://gh-proxy.com";
/// 资产下载源（镜像前缀；官方 URL 直接使用 asset.browser_download_url）
const DOWNLOAD_MIRROR_BASE: &str = "https://gh-proxy.com/https://github.com";

/// 带超时与 User-Agent 的共享 Agent（在每个源上复用）
fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(NETWORK_TIMEOUT_SECS))
        .build()
}

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
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DownloadVariant {
    /// Windows x64 CPU
    WinCpu,
    /// Windows x64 CUDA 12.4
    WinCuda124,
    /// Windows x64 CUDA 13.3
    WinCuda133,
    /// Windows x64 ROCm 7.14（携带 GPU 目标数据，如 "gfx103X"）
    WinRocm714(String),
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
    pub fn asset_name(&self) -> String {
        match self {
            DownloadVariant::WinCpu => "bin-win-cpu-x64".to_string(),
            DownloadVariant::WinCuda124 => "bin-win-cuda-12.4-x64".to_string(),
            DownloadVariant::WinCuda133 => "bin-win-cuda-13.3-x64".to_string(),
            DownloadVariant::WinRocm714(gpu_target) => {
                format!("llama-.*-windows-rocm-{}-x64\\.zip", gpu_target)
            }
            DownloadVariant::WinVulkan => "bin-win-vulkan-x64".to_string(),
            DownloadVariant::WinCpuArm64 => "bin-win-cpu-arm64".to_string(),
            DownloadVariant::LinuxCpu => "bin-ubuntu-x64".to_string(),
            DownloadVariant::LinuxVulkan => "bin-ubuntu-vulkan-x64".to_string(),
        }
    }

    /// 判断是否使用 lemonade-sdk API（ROCm 7.14 变体使用）
    pub fn is_rocm_lemonade(&self) -> bool {
        matches!(self, DownloadVariant::WinRocm714(_))
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
            "rocm714" if !is_linux => DownloadVariant::WinRocm714("gfx103X".to_string()),
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
    pub fn start_download(
        &self,
        base_dir: PathBuf,
        variant: DownloadVariant,
        release_channel: String,
    ) {
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
            .spawn(move || {
                download_in_background(base_dir, variant, release_channel, cancel, status)
            }) {
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

/// GitHub API 基址（官方直连）
fn api_base(variant: &DownloadVariant) -> &'static str {
    if variant.is_rocm_lemonade() {
        "https://api.github.com/repos/lemonade-sdk/llamacpp-rocm"
    } else {
        "https://api.github.com/repos/ggml-org/llama.cpp"
    }
}

/// 将 GitHub API URL 转换为 gh-proxy 镜像 URL
fn mirror_url(official: &str) -> String {
    match official.strip_prefix("https://api.github.com") {
        Some(rest) => format!("{}{}", API_MIRROR_BASE, rest),
        None => official.to_string(),
    }
}

/// 请求 GitHub latest release API：官方直连失败（超时/网络错误）时自动尝试镜像。
/// 全部源失败返回 ERR_NETWORK（固定标记，UI 展示"获取失败：网络错误"）。
fn fetch_release(variant: &DownloadVariant) -> Result<ReleaseInfo, String> {
    let official_url = format!("{}/releases/latest", api_base(variant));
    let mirror_url = mirror_url(&official_url);
    let ag = agent();
    for url in [official_url, mirror_url] {
        let response = match ag.get(&url).set("User-Agent", USER_AGENT).call() {
            Ok(r) => r,
            Err(_) => continue,
        };
        let body = match response.into_string() {
            Ok(b) => b,
            Err(_) => continue,
        };
        if let Ok(info) = serde_json::from_str::<ReleaseInfo>(&body) {
            return Ok(info);
        }
    }
    Err(ERR_NETWORK.to_string())
}

/// 获取指定 tag 的 release 信息（用于 stable 模式获取 nightly release）。
/// 官方直连失败（超时/网络错误）时自动尝试镜像；全部失败返回 ERR_NETWORK。
fn fetch_release_by_tag(tag: &str, is_rocm: bool) -> Result<ReleaseInfo, String> {
    let variant_base = if is_rocm {
        "https://api.github.com/repos/lemonade-sdk/llamacpp-rocm"
    } else {
        "https://api.github.com/repos/ggml-org/llama.cpp"
    };
    let official_url = format!("{}/releases/tags/{}", variant_base, tag);
    let mirror = mirror_url(&official_url);
    let ag = agent();
    for url in [official_url, mirror] {
        let response = match ag.get(&url).set("User-Agent", USER_AGENT).call() {
            Ok(r) => r,
            Err(_) => continue,
        };
        let body = match response.into_string() {
            Ok(b) => b,
            Err(_) => continue,
        };
        if let Ok(info) = serde_json::from_str::<ReleaseInfo>(&body) {
            return Ok(info);
        }
    }
    Err(ERR_NETWORK.to_string())
}

/// 从 latest release 的 assets 中下载 nightly-tag.txt 的内容
/// 返回 nightly tag 名（如 "b10549"）
/// 官方直连失败（超时/网络错误）时自动尝试镜像；全部失败返回 ERR_NETWORK。
fn fetch_nightly_tag_from_latest(variant: &DownloadVariant) -> Result<String, String> {
    let release = fetch_release(variant)?;
    // 找到 nightly-tag.txt 资产
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == "nightly-tag.txt")
        .ok_or_else(|| format!("no nightly-tag.txt asset in release {}", release.tag_name))?;
    // 下载 nightly-tag.txt 内容（官方直连 → gh-proxy 镜像依次尝试）
    let ag = agent();
    let official_url = &asset.browser_download_url;
    let mirror = match official_url.strip_prefix("https://github.com") {
        Some(rest) => format!("{}{}", DOWNLOAD_MIRROR_BASE, rest),
        None => official_url.clone(),
    };
    for url in [official_url.as_str(), mirror.as_str()] {
        let response = match ag.get(url).set("User-Agent", USER_AGENT).call() {
            Ok(r) => r,
            Err(_) => continue,
        };
        let body = match response.into_string() {
            Ok(b) => b,
            Err(_) => continue,
        };
        let tag = body.trim().to_string();
        if !tag.is_empty() {
            return Ok(tag);
        }
    }
    Err(ERR_NETWORK.to_string())
}

/// 获取最新 nightly release（preview 模式使用）
/// 通过 GitHub releases API 获取所有 releases，找到 tag 匹配 b[NUM] 格式的最新 release。
/// 官方直连失败（超时/网络错误）时自动尝试镜像；全部失败返回 ERR_NETWORK。
fn fetch_latest_nightly_release(variant: &DownloadVariant) -> Result<ReleaseInfo, String> {
    let official_url = format!("{}/releases?per_page=10", api_base(variant));
    let mirror = mirror_url(&official_url);
    let ag = agent();
    for url in [official_url.as_str(), mirror.as_str()] {
        let response = match ag.get(url).set("User-Agent", USER_AGENT).call() {
            Ok(r) => r,
            Err(_) => continue,
        };
        let body = match response.into_string() {
            Ok(b) => b,
            Err(_) => continue,
        };
        if let Ok(releases) = serde_json::from_str::<Vec<ReleaseInfo>>(&body) {
            // 找到第一个 tag 匹配 b[NUM] 格式的 release（最新的 nightly）
            if let Some(release) = releases.into_iter().find(|r| {
                let tag = &r.tag_name;
                tag.starts_with('b')
                    && tag.len() > 1
                    && tag[1..].chars().all(|c| c.is_ascii_digit())
            }) {
                return Ok(release);
            }
            // 解析成功但没找到 nightly tag，视为业务错误（非网络错误）
            return Err("no nightly release (b[NUM] tag) found".to_string());
        }
    }
    Err(ERR_NETWORK.to_string())
}

/// 获取最新 release 的 tag_name（如 "b10549"），供"检查更新"使用
/// stable: 通过 nightly-tag.txt 获取 vX.Y.Z 确认的 nightly tag
/// preview: 直接获取最新 nightly tag（b[NUM]）
pub fn fetch_latest_tag(variant: DownloadVariant, release_channel: &str) -> Result<String, String> {
    if release_channel == "stable" {
        fetch_nightly_tag_from_latest(&variant)
    } else {
        fetch_latest_nightly_release(&variant).map(|r| r.tag_name)
    }
}

// ======================= 下载流程（后台线程） =======================

/// 后台线程入口：执行完整下载流程，结果写入共享状态
pub fn download_in_background(
    base_dir: PathBuf,
    variant: DownloadVariant,
    release_channel: String,
    cancel: Arc<AtomicBool>,
    status: Arc<Mutex<DownloadStatus>>,
) {
    match run_download(&base_dir, variant, &release_channel, &cancel, &status) {
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
    release_channel: &str,
    cancel: &AtomicBool,
    status: &Arc<Mutex<DownloadStatus>>,
) -> Result<String, String> {
    // 1) 获取最新版本信息（根据发布通道）
    set_running(status, Phase::FetchingRelease, 0, None);
    let release = if release_channel == "stable" {
        // stable: 先读取 vX.Y.Z 的 nightly-tag.txt，获取确认的 nightly 版本
        let nightly_tag = fetch_nightly_tag_from_latest(&variant)?;
        fetch_release_by_tag(&nightly_tag, variant.is_rocm_lemonade())?
    } else {
        // preview: 直接获取最新 nightly release（不经过 vX.Y.Z 确认）
        fetch_latest_nightly_release(&variant)?
    };

    // 2) 按变体匹配资产
    let asset = pick_asset(&release.assets, &variant).ok_or_else(|| {
        format!(
            "no matching asset for pattern '{}' in release {}",
            variant.asset_name(),
            release.tag_name
        )
    })?;

    // 3) 流式下载（partial 文件 + rename 原子落盘）；官方/镜像两源依次尝试
    set_running(status, Phase::Downloading, 0, Some(asset.size));
    let llama_dir = base_dir.join("llama");
    fs::create_dir_all(&llama_dir).map_err(|e| format!("create dir failed: {}", e))?;
    let partial = llama_dir.join(format!(".partial.{}", asset.name));
    // 构造官方与镜像两组下载 URL
    let official_url = asset.browser_download_url.clone();
    let mirror = match official_url.strip_prefix("https://github.com") {
        Some(rest) => format!("{}{}", DOWNLOAD_MIRROR_BASE, rest),
        None => official_url.clone(),
    };
    let mut last_err = String::new();
    for url in [official_url, mirror] {
        if cancel.load(Ordering::SeqCst) {
            let _ = fs::remove_file(&partial);
            return Err("download cancelled".to_string());
        }
        let _ = fs::remove_file(&partial); // 上一源失败的残留
        match download_to_file(&url, &partial, cancel, status) {
            Ok(()) => {
                // 下载完整性校验：实际字节应与资产声明一致
                let actual_size = fs::metadata(&partial).map(|m| m.len()).unwrap_or(0);
                if actual_size != asset.size {
                    last_err = format!(
                        "incomplete download: expected {} bytes, got {}",
                        asset.size, actual_size
                    );
                    continue; // 数据不完整，尝试下一个源
                }
                last_err.clear();
                break;
            }
            Err(DlError::Cancelled) => {
                let _ = fs::remove_file(&partial);
                return Err("download cancelled".to_string());
            }
            Err(DlError::Failed(e)) => {
                last_err = e;
                continue; // 该源失败，尝试下一个
            }
        }
    }
    if !last_err.is_empty() {
        let _ = fs::remove_file(&partial);
        return Err(ERR_NETWORK.to_string());
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
/// 使用带超时的 agent，防止网络卡死。
fn download_to_file(
    url: &str,
    out: &Path,
    cancel: &AtomicBool,
    status: &Arc<Mutex<DownloadStatus>>,
) -> Result<(), DlError> {
    let ag = agent();
    let response = ag
        .get(url)
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
/// - ROCm 变体使用正则表达式匹配（pattern 含 `.*` 或 `\.`）；
/// - 其他变体使用 substring 匹配
pub fn pick_asset<'a>(assets: &'a [Asset], variant: &'a DownloadVariant) -> Option<&'a Asset> {
    let pattern = variant.asset_name();
    let ext = variant.extension();

    // 检查是否为正则模式（ROCm 变体使用正则）
    let is_regex = pattern.contains(".*") || pattern.contains("\\.");

    assets.iter().find(|a| {
        if a.name.starts_with("cudart-") {
            return false;
        }
        if !a.name.ends_with(ext) {
            return false;
        }

        if is_regex {
            // 使用正则匹配
            match regex::Regex::new(&pattern) {
                Ok(re) => re.is_match(&a.name),
                Err(_) => false,
            }
        } else {
            // 原有的 substring 匹配
            a.name.contains(&pattern)
        }
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
        let picked = pick_asset(&assets, &DownloadVariant::WinCpu).expect("应匹配 WinCpu 资产");
        assert_eq!(picked.name, "llama-b10549-bin-win-cpu-x64.zip");
    }

    #[test]
    fn pick_asset_win_cuda_versions() {
        let assets = vec![
            asset("llama-b10549-bin-win-cuda-12.4-x64.zip"),
            asset("llama-b10549-bin-win-cuda-13.3-x64.zip"),
        ];
        let p124 =
            pick_asset(&assets, &DownloadVariant::WinCuda124).expect("应匹配 CUDA 12.4 资产");
        assert_eq!(p124.name, "llama-b10549-bin-win-cuda-12.4-x64.zip");
        let p133 =
            pick_asset(&assets, &DownloadVariant::WinCuda133).expect("应匹配 CUDA 13.3 资产");
        assert_eq!(p133.name, "llama-b10549-bin-win-cuda-13.3-x64.zip");
    }

    #[test]
    fn pick_asset_win_cpu_arm64_hit() {
        let assets = vec![
            asset("llama-b10549-bin-win-cpu-x64.zip"),
            asset("llama-b10549-bin-win-cpu-arm64.zip"),
        ];
        let picked = pick_asset(&assets, &DownloadVariant::WinCpuArm64).expect("应匹配 arm64 资产");
        assert_eq!(picked.name, "llama-b10549-bin-win-cpu-arm64.zip");
        // x64 变体不应命中 arm64 资产
        assert!(pick_asset(&[assets[1].clone()], &DownloadVariant::WinCpu).is_none());
    }

    #[test]
    fn pick_asset_excludes_cudart() {
        // 仅 cudart-* 资产 → None
        let only_cudart = vec![asset("cudart-llama-b10549-bin-win-cuda-12.4-x64.zip")];
        assert!(pick_asset(&only_cudart, &DownloadVariant::WinCuda124).is_none());
        // cudart 与官方资产并存 → 选官方资产
        let mixed = vec![
            asset("cudart-llama-b10549-bin-win-cuda-12.4-x64.zip"),
            asset("llama-b10549-bin-win-cuda-12.4-x64.zip"),
        ];
        let picked =
            pick_asset(&mixed, &DownloadVariant::WinCuda124).expect("应跳过 cudart 选中官方资产");
        assert_eq!(picked.name, "llama-b10549-bin-win-cuda-12.4-x64.zip");
    }

    #[test]
    fn pick_asset_linux_cpu_vs_vulkan() {
        let assets = vec![
            asset("llama-b10549-bin-ubuntu-vulkan-x64.tar.gz"),
            asset("llama-b10549-bin-ubuntu-x64.tar.gz"),
        ];
        let cpu = pick_asset(&assets, &DownloadVariant::LinuxCpu).expect("应匹配 Linux CPU 资产");
        assert_eq!(cpu.name, "llama-b10549-bin-ubuntu-x64.tar.gz");
        let vulkan =
            pick_asset(&assets, &DownloadVariant::LinuxVulkan).expect("应匹配 Vulkan 资产");
        assert_eq!(vulkan.name, "llama-b10549-bin-ubuntu-vulkan-x64.tar.gz");
    }

    #[test]
    fn pick_asset_win_rocm_and_vulkan() {
        let assets = vec![
            asset("llama-b10549-windows-rocm-gfx1030-x64.zip"),
            asset("llama-b10549-bin-win-vulkan-x64.zip"),
            asset("llama-b10549-bin-win-cuda-13.3-x64.zip"),
        ];
        let rocm_variant = DownloadVariant::WinRocm714("gfx1030".to_string());
        let rocm = pick_asset(&assets, &rocm_variant).expect("应匹配 ROCm 7.14 资产");
        assert_eq!(rocm.name, "llama-b10549-windows-rocm-gfx1030-x64.zip");
        let vulkan =
            pick_asset(&assets, &DownloadVariant::WinVulkan).expect("应匹配 Win Vulkan 资产");
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
                DownloadVariant::WinRocm714("gfx103X".to_string())
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
        assert!(pick_asset(&assets, &DownloadVariant::LinuxVulkan).is_none());
        assert!(pick_asset(&[], &DownloadVariant::WinCpu).is_none());
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

    #[test]
    fn test_win_rocm714_asset_name() {
        let variant = DownloadVariant::WinRocm714("gfx103X".to_string());
        let asset_name = variant.asset_name();
        assert!(asset_name.contains("llama-.*-windows-rocm-gfx103X-x64\\.zip"));
    }

    #[test]
    fn test_from_settings_value_rocm714() {
        let variant = DownloadVariant::from_settings_value("rocm714");
        match variant {
            DownloadVariant::WinRocm714(gpu_target) => {
                assert_eq!(gpu_target, "gfx103X");
            }
            _ => panic!("Expected WinRocm714 variant"),
        }
    }

    #[test]
    fn test_pick_asset_rocm714() {
        let assets = vec![
            Asset {
                name: "llama-b1313-windows-rocm-gfx103X-x64.zip".to_string(),
                size: 1000,
                browser_download_url:
                    "https://example.com/llama-b1313-windows-rocm-gfx103X-x64.zip".to_string(),
            },
            Asset {
                name: "llama-b1313-windows-rocm-gfx110X-x64.zip".to_string(),
                size: 1000,
                browser_download_url:
                    "https://example.com/llama-b1313-windows-rocm-gfx110X-x64.zip".to_string(),
            },
        ];
        let variant = DownloadVariant::WinRocm714("gfx103X".to_string());
        let picked = pick_asset(&assets, &variant);
        assert!(picked.is_some());
        assert_eq!(
            picked.unwrap().name,
            "llama-b1313-windows-rocm-gfx103X-x64.zip"
        );
    }
}
