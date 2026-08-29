use crate::config::settings::AppSettings;
use crate::engine::ErrorInfo;
use crate::i18n;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command};
use std::sync::{Arc, Mutex};
use std::thread;

pub(crate) const MAX_LOG_LINES: usize = 10_000; // 日志环形缓冲区最大行数

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[derive(Debug, Clone, PartialEq)]
pub enum ServerState {
    Idle,
    Starting,
    Running,
    Stopping,
    Error(ErrorInfo),
}

#[derive(Debug, Clone, PartialEq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub text: String,
    pub level: LogLevel,
}

struct InnerState {
    child: Option<Child>,
    logs: VecDeque<LogEntry>,
    progress: f32, // 预填充进度 0.0~1.0
}

pub struct ServerManager {
    state: ServerState,
    inner: Arc<Mutex<InnerState>>,
    launch_command: Option<String>,
    _threads: Vec<thread::JoinHandle<()>>,
}

impl Default for ServerManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerManager {
    pub fn new() -> Self {
        Self {
            state: ServerState::Idle,
            inner: Arc::new(Mutex::new(InnerState {
                child: None,
                logs: VecDeque::new(),
                progress: 0.0,
            })),
            launch_command: None,
            _threads: Vec::new(),
        }
    }

    pub fn state(&self) -> ServerState {
        self.state.clone()
    }

    pub fn is_running(&self) -> bool {
        matches!(self.state, ServerState::Running)
    }

    pub fn status_text(&self, lang: &i18n::Language) -> String {
        match &self.state {
            ServerState::Idle => i18n::t(i18n::Key::StatusIdle, lang).to_string(),
            ServerState::Starting => i18n::t(i18n::Key::StatusStarting, lang).to_string(),
            ServerState::Running => i18n::t(i18n::Key::StatusRunning, lang).to_string(),
            ServerState::Stopping => i18n::t(i18n::Key::StatusStopping, lang).to_string(),
            ServerState::Error(err) => {
                format!(
                    "{}: {}",
                    i18n::t(i18n::Key::StatusError, lang),
                    err.text(lang)
                )
            }
        }
    }

    // 对外仍返回 Vec，内部使用 VecDeque 作环形缓冲
    pub fn logs(&self) -> Vec<LogEntry> {
        let inner = self.inner.lock().unwrap();
        inner.logs.iter().cloned().collect()
    }

    // 判断 Server 是否已输出 "llama_server: listening on"（表示真正就绪）
    pub fn is_listening(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        inner
            .logs
            .iter()
            .any(|e| e.text.contains("llama_server: listening on"))
    }

    pub fn clear_logs(&mut self) {
        self.inner.lock().unwrap().logs.clear();
        self.inner.lock().unwrap().progress = 0.0;
    }

    pub fn progress(&self) -> f32 {
        self.inner.lock().unwrap().progress
    }

    /// 基于时间戳+位置的单字母标识符检测日志等级
    pub(crate) fn detect_log_level(line: &str) -> Option<LogLevel> {
        let line = line.trim_start();

        // 必须以数字开头（类似时间戳）
        if !line
            .chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false)
        {
            return None;
        }

        // 找到第一个空格，前面视为时间戳段
        let Some(first_space) = line.find(' ') else {
            return None;
        };

        let ts_part = &line[..first_space];

        // 时间戳段只允许数字和点
        if ts_part.chars().any(|c| !(c.is_ascii_digit() || c == '.')) {
            return None;
        }

        // 至少两个点，看起来更像时间戳而不是普通数字
        let dot_count = ts_part.chars().filter(|&c| c == '.').count();
        if dot_count < 2 {
            return None;
        }

        // 时间戳后是单字母等级标识符 I / W / E，后面接空格或结尾
        let rest = &line[first_space + 1..];
        if rest.is_empty() {
            return None;
        }

        match rest.as_bytes()[0] {
            b'I' if (rest.len() == 1 || rest.as_bytes().get(1).is_some_and(|&b| b == b' ')) => {
                return Some(LogLevel::Info);
            }
            b'W' if (rest.len() == 1 || rest.as_bytes().get(1).is_some_and(|&b| b == b' ')) => {
                return Some(LogLevel::Warn);
            }
            b'E' if (rest.len() == 1 || rest.as_bytes().get(1).is_some_and(|&b| b == b' ')) => {
                return Some(LogLevel::Error);
            }
            _ => {}
        };

        None
    }

    // 从日志文本中解析 progress = 0.xx，并更新进度值
    // 不修改原始日志内容，只提取进度
    fn parse_progress(text: &str) -> (String, Option<f32>) {
        let mut progress = None;

        if let Some(pos) = text.find("progress = ") {
            let rest = &text[pos + "progress = ".len()..];
            // 取所有终止符中的最小位置（避免空格/逗号优先级问题）
            let end = [
                rest.find(' '),
                rest.find('\t'),
                rest.find(','),
                rest.find('\n'),
            ]
            .into_iter()
            .flatten()
            .min()
            .unwrap_or(rest.len());
            let num_str = rest[..end].trim();

            if let Ok(v) = num_str.parse::<f32>() {
                progress = Some(v.clamp(0.0, 1.0));
            }
        }

        // 保留原始日志不做裁剪
        (text.to_string(), progress)
    }

    pub fn launch_command(&self) -> Option<String> {
        self.launch_command.clone()
    }

    /// 检查 llama-server 文件是否存在
    pub fn check_server(&self, path: &std::path::Path) -> bool {
        if path.as_os_str().is_empty() {
            return false;
        }
        std::path::Path::new(path).exists()
    }

    /// 对 llama-server 文件授权读写权限（Linux 专用）
    #[cfg(target_os = "linux")]
    pub fn authorize_server(&self, path: &std::path::Path) -> Result<(), String> {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;

        if path.as_os_str().is_empty() {
            return Err("llama-server 路径为空".to_string());
        }

        if !path.exists() {
            return Err("llama-server 文件不存在".to_string());
        }

        // 设置读写执行权限 (rwxr-xr-x = 0o755)
        let perms = fs::Permissions::from_mode(0o755);
        fs::set_permissions(path, perms)
            .map_err(|e| format!("设置 llama-server 权限失败：{}", e))?;

        Ok(())
    }

    pub fn start(&mut self, settings: &AppSettings) {
        if self.is_running() {
            return;
        }

        let server_path = settings.server_path.clone();
        let model_path = settings.model_path.clone();

        if server_path.as_os_str().is_empty() || model_path.as_os_str().is_empty() {
            self.state = ServerState::Error(ErrorInfo::Key(i18n::Key::ErrServerModelMissing));
            return;
        }

        if settings.port == settings.rpc_port {
            self.state = ServerState::Error(ErrorInfo::Key(i18n::Key::ErrPortConflict));
            return;
        }

        self.state = ServerState::Starting;
        self.clear_logs();
        self.launch_command = None;
        self._threads.clear();

        let mut cmd = Command::new(&server_path);
        cmd.arg("--model")
            .arg(&model_path)
            .arg("--host")
            .arg(&settings.host)
            .arg("--port")
            .arg(settings.port.to_string())
            .arg("--ctx-size")
            .arg(settings.context_actual().to_string())
            .arg("--parallel")
            .arg(settings.parallel_slots.to_string())
            .arg("--batch-size")
            .arg(settings.batch_size_actual().to_string())
            .arg("--ubatch-size")
            .arg(settings.ubatch_size_actual().to_string())
            .arg("--timeout")
            .arg(settings.session_timeout.to_string())
            .arg("--gpu-layers")
            .arg(settings.gpu_layers_mode.to_arg());

        // 采样参数（根据启用标志决定是否拼接；ignore_* 系列为反向语义）
        if settings.enable_temperature {
            cmd.arg("--temperature")
                .arg(settings.temperature.to_string());
        }
        if settings.enable_top_p {
            cmd.arg("--top-p").arg(settings.top_p.to_string());
        }
        if settings.enable_top_k {
            cmd.arg("--top-k").arg(settings.top_k.to_string());
        }
        if settings.enable_repeat_penalty {
            cmd.arg("--repeat-penalty")
                .arg(settings.repeat_penalty.to_string());
        }
        if settings.enable_presence_penalty {
            cmd.arg("--presence-penalty")
                .arg(settings.presence_penalty.to_string());
        }

        // 思考参数（--reasoning 系列，非默认值才拼接）
        if !settings.reasoning.is_empty() && settings.reasoning != "auto" {
            cmd.arg("--reasoning").arg(&settings.reasoning);
        }
        if !settings.reasoning_format.is_empty() && settings.reasoning_format != "auto" {
            cmd.arg("--reasoning-format")
                .arg(&settings.reasoning_format);
        }
        if !settings.reasoning_effort.is_empty() && settings.reasoning_effort != "default" {
            cmd.arg("--chat-template-kwargs")
                .arg(&settings.reasoning_effort);
        }
        if settings.reasoning_budget != -1 {
            cmd.arg("--reasoning-budget")
                .arg(settings.reasoning_budget.to_string());
        }
        // 思考保留开关：开启 --reasoning-preserve，关闭 --no-reasoning-preserve，模型默认不拼接
        if settings.reasoning_preserve == Some(true) {
            cmd.arg("--reasoning-preserve");
        } else if settings.reasoning_preserve == Some(false) {
            cmd.arg("--no-reasoning-preserve");
        }

        // 会话模板参数
        if !settings.chat_template.is_empty() {
            cmd.arg("--chat-template").arg(&settings.chat_template);
        }
        if !settings.chat_template_file.as_os_str().is_empty() {
            cmd.arg("--chat-template-file")
                .arg(&settings.chat_template_file);
        }
        // Jinja 对话模板引擎开关：启用用 --jinja，禁用用 --no-jinja
        if settings.jinja_enabled {
            cmd.arg("--jinja");
        } else {
            cmd.arg("--no-jinja");
        }

        // 采样器扩展（llama.cpp b10488+ 新采样器）
        if settings.enable_min_p && settings.min_p > 0.0 {
            cmd.arg("--min-p").arg(settings.min_p.to_string());
        }
        if settings.enable_top_n_sigma && settings.top_n_sigma > 0.0 {
            cmd.arg("--top-n-sigma")
                .arg(settings.top_n_sigma.to_string());
        }
        if settings.enable_xtc && settings.xtc_probability > 0.0 {
            cmd.arg("--xtc-probability")
                .arg(settings.xtc_probability.to_string());
            if settings.xtc_threshold < 1.0 {
                cmd.arg("--xtc-threshold")
                    .arg(settings.xtc_threshold.to_string());
            }
        }
        if settings.enable_typical_p && settings.typical_p < 1.0 {
            cmd.arg("--typical-p").arg(settings.typical_p.to_string());
        }
        if settings.enable_mirostat && settings.mirostat != 0 {
            cmd.arg("--mirostat").arg(settings.mirostat.to_string());
            if settings.mirostat_lr != 0.10 {
                cmd.arg("--mirostat-lr")
                    .arg(settings.mirostat_lr.to_string());
            }
            if settings.mirostat_ent != 5.00 {
                cmd.arg("--mirostat-ent")
                    .arg(settings.mirostat_ent.to_string());
            }
        }
        if settings.enable_dynatemp && settings.dynatemp_range > 0.0 {
            cmd.arg("--dynatemp-range")
                .arg(settings.dynatemp_range.to_string());
            if settings.dynatemp_exp != 1.0 {
                cmd.arg("--dynatemp-exp")
                    .arg(settings.dynatemp_exp.to_string());
            }
        }
        if !settings.sampler_seq.is_empty() {
            cmd.arg("--sampler-seq").arg(&settings.sampler_seq);
        }

        // Flash Attention
        if !settings.flash_attn.is_empty() {
            cmd.arg("--flash-attn").arg(&settings.flash_attn);
        }

        // 多模态投影
        if !settings.mmproj_path.as_os_str().is_empty() {
            cmd.arg("--mmproj").arg(&settings.mmproj_path);
        }

        // 模型别名
        if !settings.alias.is_empty() {
            cmd.arg("--alias").arg(&settings.alias);
        }

        // DFlash / Speculative Decoding 参数整合
        let dflash_configured = !settings.dflash_path.as_os_str().is_empty();

        // 1) --model-draft: 如果配置了 DFlash，始终写入
        if dflash_configured {
            cmd.arg("--model-draft").arg(&settings.dflash_path);
        }

        // 2) --spec-type: 仅当用户明确选择非 none 时写入，不再自动 fallback dflash
        if settings.spec_type != "none" {
            cmd.arg("--spec-type").arg(&settings.spec_type);
        }

        // 3) --spec-draft-*: 仅在 spec_type != "none" 时写入
        if settings.spec_type != "none" {
            cmd.arg("--spec-draft-n-max")
                .arg(settings.spec_draft_n_max.to_string());
            cmd.arg("--spec-draft-n-min")
                .arg(settings.spec_draft_n_min.to_string());
            cmd.arg("--spec-draft-p-min")
                .arg(format!("{}", settings.spec_draft_p_min));
            cmd.arg("--spec-draft-p-split")
                .arg(format!("{}", settings.spec_draft_p_split));
            if !settings.spec_draft_type_k.is_empty() {
                cmd.arg("--spec-draft-type-k")
                    .arg(&settings.spec_draft_type_k);
            }
            if !settings.spec_draft_type_v.is_empty() {
                cmd.arg("--spec-draft-type-v")
                    .arg(&settings.spec_draft_type_v);
            }
        }

        // KV 缓存配置
        if settings.kv_offload {
            cmd.arg("--kv-offload");
        } else {
            cmd.arg("--no-kv-offload");
        }
        if !settings.cache_type_k.is_empty() {
            cmd.arg("--cache-type-k").arg(&settings.cache_type_k);
        }
        if !settings.cache_type_v.is_empty() {
            cmd.arg("--cache-type-v").arg(&settings.cache_type_v);
        }
        // 模型加载模式（--load-mode 替代已废弃的 --mmap/--mlock）
        // 仅非 auto 时拼接；auto 不拼接任何参数，由 llama-server 使用默认行为
        if !settings.load_mode.is_empty() && settings.load_mode != "auto" {
            cmd.arg("--load-mode").arg(&settings.load_mode);
        }
        if !settings.tensor_read_lazy.is_empty() && settings.tensor_read_lazy != "auto" {
            cmd.arg("--tensor-read-lazy").arg(&settings.tensor_read_lazy);
        }
        if settings.kv_unified {
            cmd.arg("--kv-unified");
        }
        if settings.swa_full {
            cmd.arg("--swa-full");
        }

        // 上下文检查点
        if settings.ctx_checkpoints != 32 {
            cmd.arg("--ctx-checkpoints")
                .arg(settings.ctx_checkpoints.to_string());
        }

        // 最小检查点步长
        if settings.checkpoint_min_step != 512 {
            cmd.arg("--checkpoint-min-step")
                .arg(settings.checkpoint_min_step.to_string());
        }

        // GPU 与设备分配
        if !settings.split_mode.is_empty() && settings.split_mode != "layer" {
            cmd.arg("--split-mode").arg(&settings.split_mode);
        }
        if !settings.tensor_split.is_empty() {
            cmd.arg("--tensor-split").arg(&settings.tensor_split);
        }
        if settings.cpu_moe {
            cmd.arg("--cpu-moe");
        }
        // 关闭 cpu_moe 时才拼接 --n-cpu-moe
        if !settings.cpu_moe && settings.n_cpu_moe > 0 {
            cmd.arg("--n-cpu-moe").arg(settings.n_cpu_moe.to_string());
        }
        // 指定特定张量到缓冲区（非空才拼接）
        if !settings.override_tensor.is_empty() {
            cmd.arg("--override-tensor").arg(&settings.override_tensor);
        }

        // CPU 线程数（llama.cpp b10488+；-1 = 不拼接沿用默认）
        if settings.threads >= 0 {
            cmd.arg("--threads").arg(settings.threads.to_string());
        }
        if settings.threads_batch >= 0 {
            cmd.arg("--threads-batch")
                .arg(settings.threads_batch.to_string());
        }
        // 生成长度上限（-1 = 不拼接 = 无限生成）
        if settings.n_predict >= 0 {
            cmd.arg("--n-predict").arg(settings.n_predict.to_string());
        }
        // 保留前缀 token 数
        if settings.keep > 0 {
            cmd.arg("--keep").arg(settings.keep.to_string());
        }
        // 随机种子（-1 = 不拼接 = 随机）
        if settings.seed >= 0 {
            cmd.arg("--seed").arg(settings.seed.to_string());
        }
        // 主 GPU（多卡时指定）
        if settings.main_gpu != 0 {
            cmd.arg("--main-gpu").arg(settings.main_gpu.to_string());
        }
        // 长上下文 / 提示缓存
        if settings.cache_prompt {
            cmd.arg("--cache-prompt");
        } else {
            cmd.arg("--no-cache-prompt");
        }
        if settings.cache_reuse > 0 {
            cmd.arg("--cache-reuse")
                .arg(settings.cache_reuse.to_string());
        }
        if settings.context_shift {
            cmd.arg("--context-shift");
        }
        // 结构化输出（JSON Schema / Grammar）
        if !settings.json_schema.is_empty() {
            cmd.arg("--json-schema").arg(&settings.json_schema);
        }
        if !settings.grammar.is_empty() {
            cmd.arg("--grammar").arg(&settings.grammar);
        }

        if settings.verbose {
            cmd.arg("--verbose");
        }

        // 日志时间戳
        if settings.log_timestamps {
            cmd.arg("--log-timestamps");
        } else {
            cmd.arg("--no-log-timestamps");
        }

        // 日志级别（0=generic 1=error 2=warning 3=info 4=trace 5=debug）
        cmd.arg("--log-verbosity")
            .arg(settings.log_verbosity.to_string());

        // 离线模式：拼接 --offline（如 llama.cpp 支持）
        if settings.offline_mode {
            cmd.arg("--offline");
        }

        // RPC 模式
        if settings.rpc_mode {
            cmd.arg("--rpc").arg(&settings.rpc_endpoints);
        }

        // 网页客户端开关：启用用 --webui，禁用用 --no-webui
        if settings.web_ui_enabled {
            cmd.arg("--webui");
        } else {
            cmd.arg("--no-webui");
        }

        // MCP 工具：生成"当前启用"的 MCP 配置文件并传给 llama-server
        // （无启用 server 或生成失败时函数返回 None，不拼接参数）
        if let Some(mcp_config_path) = settings.write_effective_mcp_config() {
            cmd.arg("--mcp-servers-config").arg(&mcp_config_path);
        }

        // API 安全 / 部署（空值不拼接）
        if !settings.api_key.is_empty() {
            cmd.arg("--api-key").arg(&settings.api_key);
        }
        if !settings.api_prefix.is_empty() {
            cmd.arg("--api-prefix").arg(&settings.api_prefix);
        }
        if !settings.cors_origins.is_empty() {
            cmd.arg("--cors-origins").arg(&settings.cors_origins);
        }
        if !settings.ssl_cert_file.as_os_str().is_empty() {
            cmd.arg("--ssl-cert-file").arg(&settings.ssl_cert_file);
        }
        if !settings.ssl_key_file.as_os_str().is_empty() {
            cmd.arg("--ssl-key-file").arg(&settings.ssl_key_file);
        }
        if settings.reuse_port {
            cmd.arg("--reuse-port");
        }
        if !settings.numa.is_empty() {
            cmd.arg("--numa").arg(&settings.numa);
        }

        // 记录启动命令
        let cmd_str = format!(
            "{} {}",
            server_path.display(),
            cmd.get_args()
                .map(|a| a.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" ")
        );

        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        // Windows: 隐藏子进程的命令行窗口
        #[cfg(target_os = "windows")]
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

        match cmd.spawn() {
            Ok(child) => {
                {
                    let mut inner = self.inner.lock().unwrap();
                    inner.child = Some(child);
                }
                self.launch_command = Some(cmd_str);

                let inner_clone = Arc::clone(&self.inner);
                let stdout_thread = thread::spawn(move || {
                    let stdout = {
                        let mut inner = inner_clone.lock().unwrap();
                        if let Some(ref mut child) = inner.child {
                            child.stdout.take()
                        } else {
                            None
                        }
                    };
                    if let Some(stdout) = stdout {
                        let reader = BufReader::new(stdout);
                        for line in reader.lines() {
                            match line {
                                Ok(l) => {
                                    // 优先使用基于时间戳+位置的单字母等级检测
                                    let level = match Self::detect_log_level(&l) {
                                        Some(level) => level,
                                        None => {
                                            if l.contains("WARN") || l.contains("warn") {
                                                LogLevel::Warn
                                            } else if l.contains("ERROR") || l.contains("error") {
                                                LogLevel::Error
                                            } else {
                                                LogLevel::Info
                                            }
                                        }
                                    };

                                    let (text, p) = Self::parse_progress(&l);
                                    let mut inner = inner_clone.lock().unwrap();
                                    if let Some(v) = p {
                                        inner.progress = v;
                                    }
                                    // 超过上限时丢弃最旧的一行
                                    if inner.logs.len() >= MAX_LOG_LINES {
                                        inner.logs.pop_front();
                                    }
                                    inner.logs.push_back(LogEntry { text, level });
                                }
                                Err(_) => break,
                            }
                        }
                    }
                });

                let inner_clone2 = Arc::clone(&self.inner);
                let stderr_thread = thread::spawn(move || {
                    let stderr = {
                        let mut inner = inner_clone2.lock().unwrap();
                        if let Some(ref mut child) = inner.child {
                            child.stderr.take()
                        } else {
                            None
                        }
                    };
                    if let Some(stderr) = stderr {
                        let reader = BufReader::new(stderr);
                        for line in reader.lines() {
                            match line {
                                Ok(l) => {
                                    // 优先使用基于时间戳+位置的单字母等级检测
                                    let level = match Self::detect_log_level(&l) {
                                        Some(level) => level,
                                        None => {
                                            if l.contains("WARN") || l.contains("warn") {
                                                LogLevel::Warn
                                            } else if l.contains("ERROR") || l.contains("error") {
                                                LogLevel::Error
                                            } else {
                                                LogLevel::Info
                                            }
                                        }
                                    };
                                    let (text, p) = Self::parse_progress(&l);
                                    let mut inner = inner_clone2.lock().unwrap();
                                    if let Some(v) = p {
                                        inner.progress = v;
                                    }
                                    // 超过上限时丢弃最旧的一行
                                    if inner.logs.len() >= MAX_LOG_LINES {
                                        inner.logs.pop_front();
                                    }
                                    inner.logs.push_back(LogEntry { text, level });
                                }
                                Err(_) => break,
                            }
                        }
                    }
                });

                self._threads.push(stdout_thread);
                self._threads.push(stderr_thread);
            }
            Err(e) => {
                self.state = ServerState::Error(ErrorInfo::WithDetail(
                    i18n::Key::ErrStartFailed,
                    e.to_string(),
                ));
                self.launch_command = None;
            }
        }
    }

    pub fn stop(&mut self) {
        if let Some(mut child) = self.inner.lock().unwrap().child.take() {
            self.state = ServerState::Stopping;
            let _ = child.kill();
            let _ = child.wait();
            self.state = ServerState::Idle;
        }
        self.launch_command = None;
        self._threads.clear();
    }

    pub fn poll_logs(&mut self) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(ref mut child) = inner.child {
            if let Ok(Some(status)) = child.try_wait() {
                let exit_msg = if status.success() {
                    i18n::t(i18n::Key::StatusServerExited, &i18n::Language::En).to_string()
                } else {
                    format!(
                        "{}: {:?}",
                        i18n::t(i18n::Key::StatusServerCrashed, &i18n::Language::En),
                        status.code()
                    )
                };
                // 超过上限时丢弃最旧的一行
                if inner.logs.len() >= MAX_LOG_LINES {
                    inner.logs.pop_front();
                }
                inner.logs.push_back(LogEntry {
                    text: exit_msg,
                    level: LogLevel::Warn,
                });
                self.state = ServerState::Idle;
                // 清除 child，防止每帧重复添加崩溃日志
                inner.child = None;
            }
        }
        drop(inner);

        if matches!(self.state, ServerState::Starting) {
            self.state = ServerState::Running;
        }
    }
}

impl Drop for ServerManager {
    fn drop(&mut self) {
        self.stop();
    }
}
