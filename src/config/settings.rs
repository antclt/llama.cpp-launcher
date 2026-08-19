use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const CONFIG_FILE: &str = "llama_cpp_launcher_settings.json";

/// GPU 层数卸载模式
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GpuLayersMode {
    Auto,          // 自动
    All,           // 全部卸载到 GPU
    Manual(usize), // 手动指定层数
}

impl serde::Serialize for GpuLayersMode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            GpuLayersMode::Auto => serializer.serialize_str("auto"),
            GpuLayersMode::All => serializer.serialize_str("all"),
            GpuLayersMode::Manual(n) => n.serialize(serializer),
        }
    }
}

impl<'de> serde::Deserialize<'de> for GpuLayersMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de;

        struct GpuLayersModeVisitor;

        impl<'de> de::Visitor<'de> for GpuLayersModeVisitor {
            type Value = GpuLayersMode;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("auto, all, or a number")
            }

            fn visit_str<E>(self, value: &str) -> Result<GpuLayersMode, E>
            where
                E: de::Error,
            {
                let v = value.trim().to_lowercase();
                if v == "auto" {
                    Ok(GpuLayersMode::Auto)
                } else if v == "all" || v == "999" {
                    Ok(GpuLayersMode::All)
                } else if let Ok(n) = v.parse::<usize>() {
                    Ok(GpuLayersMode::Manual(n))
                } else {
                    Err(de::Error::invalid_value(de::Unexpected::Str(value), &self))
                }
            }

            fn visit_u64<E>(self, v: u64) -> Result<GpuLayersMode, E>
            where
                E: de::Error,
            {
                Ok(GpuLayersMode::Manual(v as usize))
            }
        }

        deserializer.deserialize_any(GpuLayersModeVisitor)
    }
}

impl GpuLayersMode {
    /// 生成 --gpu-layers 参数值
    pub fn to_arg(&self) -> String {
        match self {
            GpuLayersMode::Auto => "auto".to_string(),
            GpuLayersMode::All => "256".to_string(),
            GpuLayersMode::Manual(n) => n.to_string(),
        }
    }
}

fn default_flash_attn() -> String {
    "auto".to_string()
}

// 思考控制（Reasoning / Thinking）默认值
fn default_reasoning_mode() -> String {
    "auto".to_string() // --reasoning auto/on/off，auto = 按模板自动
}

fn default_reasoning_effort() -> String {
    "default".to_string() // --reasoning-effort，"default" = 保持模板默认（不拼接）
}

fn default_reasoning_format() -> String {
    "auto".to_string() // --reasoning-format auto/none/deepseek/deepseek-legacy
}

fn default_reasoning_budget() -> i64 {
    -1 // --reasoning-budget，-1 = 不限制（不拼接）
}

fn default_jinja_enabled() -> bool {
    true // --jinja / --no-jinja
}

fn default_load_mode() -> String {
    "auto".to_string() // --load-mode，"auto" = 不拼接并沿用旧版 --mmap/--mlock
}

fn default_web_ui_enabled() -> bool {
    true
}

fn default_log_timestamps() -> bool {
    true
}

fn default_session_timeout() -> usize {
    600 // 会话超时（秒）默认值
}

fn default_auto_scroll_logs() -> bool {
    true
}

fn default_max_log_lines() -> i32 {
    100
}

fn default_log_to_file() -> bool {
    false
}

fn default_dark_mode() -> bool {
    true
}

fn default_theme_mode() -> String {
    "auto".to_string()
}

fn default_accent_color() -> String {
    "#FF2D55".to_string()
}

// context / batch_size / ubatch_size 以 k 为单位存储 (1k = 1024)
// 反序列化时兼容旧版原始值（如 4096 → 自动转为 4）

fn default_context() -> usize {
    4 // 4k = 4096
}

fn default_batch_size() -> usize {
    2 // 2k = 2048
}

fn default_ubatch_size() -> f32 {
    0.5 // 0.5k = 512
}

/// 将旧版原始值（如 4096）转换为 k 单位，若已是小数值则视为 k 单位
fn from_raw_or_k(v: usize) -> usize {
    if v >= 128 {
        (v + 512) / 1024 // 向上取整到最近的 k
    } else {
        v.max(1)
    }
}

mod deserialize_context {
    use super::from_raw_or_k;
    use serde::{self, Deserialize};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<usize, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = usize::deserialize(deserializer)?;
        Ok(from_raw_or_k(v))
    }
}

mod deserialize_batch_size {
    use super::from_raw_or_k;
    use serde::{self, Deserialize};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<usize, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = usize::deserialize(deserializer)?;
        Ok(from_raw_or_k(v))
    }
}

mod deserialize_ubatch_size {
    use serde::{self, Deserialize};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<f32, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // 兼容旧版整数格式（如 1 → 1.0）和浮点格式（如 0.5）
        let v = serde_json::Value::deserialize(deserializer)?;
        match v.as_f64() {
            Some(n) => {
                let val = n as f32;
                // 若为较大原始值（如 1024），转换为 k 单位
                if val >= 128.0 {
                    Ok((val / 1024.0).max(0.5))
                } else {
                    Ok(val.max(0.5))
                }
            }
            None => Ok(0.5),
        }
    }
}

// 推测解码（Speculative Decoding）默认值
fn default_spec_type() -> String {
    "none".to_string()
}

fn default_spec_draft_n_max() -> usize {
    3
}

fn default_spec_draft_p_min() -> f32 {
    0.75
}

fn default_spec_draft_p_split() -> f32 {
    1.0
}

// KV 缓存比例默认值
fn default_kv_cache_ratio() -> f32 {
    0.95
}

// 上下文检查点默认值
fn default_ctx_checkpoints() -> usize {
    32
}

// 最小检查点步长默认值
fn default_checkpoint_min_step() -> usize {
    512
}

// Duplicate definition removed - keeping only one instance above
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    // Server 配置
    pub host: String,
    pub port: u16,
    pub parallel_slots: usize,
    // 推理参数（以 k 为单位存储，1k = 1024）
    #[serde(
        default = "default_context",
        deserialize_with = "deserialize_context::deserialize"
    )]
    pub context: usize, // --ctx-size (k)
    #[serde(
        default = "default_batch_size",
        deserialize_with = "deserialize_batch_size::deserialize"
    )]
    pub batch_size: usize, // --batch-size (k)
    #[serde(
        default = "default_ubatch_size",
        deserialize_with = "deserialize_ubatch_size::deserialize"
    )]
    pub ubatch_size: f32, // --ubatch-size (k, 0.5 步进)
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: i32,
    pub repeat_penalty: f32,
    pub presence_penalty: f32,
    // 采样参数启用标志（勾选时拼接启动命令）
    #[serde(default)]
    pub enable_temperature: bool,
    #[serde(default)]
    pub enable_top_p: bool,
    #[serde(default)]
    pub enable_top_k: bool,
    #[serde(default)]
    pub enable_repeat_penalty: bool,
    #[serde(default)]
    pub enable_presence_penalty: bool,
    #[serde(default = "default_flash_attn")]
    pub flash_attn: String,

    // 思考控制（Reasoning / Thinking）配置
    #[serde(default = "default_reasoning_mode")]
    pub reasoning_mode: String, // --reasoning (auto/on/off)
    #[serde(default = "default_reasoning_effort")]
    pub reasoning_effort: String, // --reasoning-effort ("default" 时不拼接)
    #[serde(default = "default_reasoning_format")]
    pub reasoning_format: String, // --reasoning-format (auto/none/deepseek/deepseek-legacy)
    #[serde(default)]
    pub reasoning_preserve: String, // "" / "on" / "off" → 不传 / --reasoning-preserve / --no-reasoning-preserve
    #[serde(default = "default_reasoning_budget")]
    pub reasoning_budget: i64, // --reasoning-budget (-1 不拼接)
    #[serde(default)]
    pub reasoning_budget_message: String, // --reasoning-budget-message (空不拼接)

    // 聊天模板
    #[serde(default = "default_jinja_enabled")]
    pub jinja_enabled: bool, // --jinja / --no-jinja
    #[serde(default)]
    pub chat_template_file: PathBuf, // --chat-template-file (空不拼接)

    // 加载模式（新版 --load-mode；"auto" 时沿用旧版 --mmap/--mlock 行为）
    #[serde(default = "default_load_mode")]
    pub load_mode: String, // --load-mode

    // 推测解码（Speculative Decoding）配置
    #[serde(default = "default_spec_type")]
    pub spec_type: String, // --spec-type
    #[serde(default = "default_spec_draft_n_max")]
    pub spec_draft_n_max: usize, // --spec-draft-n-max
    #[serde(default)]
    pub spec_draft_n_min: usize, // --spec-draft-n-min
    #[serde(default = "default_spec_draft_p_min")]
    pub spec_draft_p_min: f32, // --spec-draft-p-min
    #[serde(default = "default_spec_draft_p_split")]
    pub spec_draft_p_split: f32, // --spec-draft-p-split

    // KV 缓存配置
    pub kv_offload: bool,
    pub cache_type_k: String,
    pub cache_type_v: String,
    pub kv_mlock: bool,   // --mlock
    pub kv_mmap: bool,    // --mmap / --no-mmap
    pub kv_unified: bool, // --kv-unified
    #[serde(default)]
    pub swa_full: bool, // --swa-full
    #[serde(default = "default_kv_cache_ratio")]
    pub kv_cache_ratio: f32, // KV 缓存比例 (不拼接启动命令)
    #[serde(default = "default_ctx_checkpoints")]
    pub ctx_checkpoints: usize, // --ctx-checkpoints
    #[serde(default = "default_checkpoint_min_step")]
    pub checkpoint_min_step: usize, // --checkpoint-min-step
    // GPU 与设备分配
    pub gpu_layers_mode: GpuLayersMode,
    pub split_mode: String,
    pub tensor_split: String,
    pub cpu_moe: bool,
    pub n_cpu_moe: usize,
    // 高级
    pub verbose: bool,
    // 离线模式
    #[serde(default)]
    pub offline_mode: bool,

    // RPC 模式
    pub rpc_mode: bool,
    pub rpc_endpoints: String,

    // 网页客户端开关
    #[serde(default = "default_web_ui_enabled")]
    pub web_ui_enabled: bool,

    // 原生日志时间戳
    #[serde(default = "default_log_timestamps")]
    pub log_timestamps: bool,

    // 会话超时（秒）
    #[serde(default = "default_session_timeout")]
    pub session_timeout: usize,
}

impl Default for Preset {
    fn default() -> Self {
        Self {
            name: String::new(),
            host: "127.0.0.1".to_string(),
            port: 8080,
            parallel_slots: 1,
            context: 4,       // 4k = 4096
            batch_size: 2,    // 2k = 2048
            ubatch_size: 0.5, // 0.5k = 512
            temperature: 0.8,
            top_p: 0.95,
            top_k: 40,
            repeat_penalty: 1.1,
            presence_penalty: 0.0,
            enable_temperature: false,
            enable_top_p: false,
            enable_top_k: false,
            enable_repeat_penalty: false,
            enable_presence_penalty: false,
            flash_attn: default_flash_attn(),
            reasoning_mode: default_reasoning_mode(),
            reasoning_effort: default_reasoning_effort(),
            reasoning_format: default_reasoning_format(),
            reasoning_preserve: String::new(),
            reasoning_budget: default_reasoning_budget(),
            reasoning_budget_message: String::new(),
            jinja_enabled: default_jinja_enabled(),
            chat_template_file: PathBuf::new(),
            load_mode: default_load_mode(),
            spec_type: default_spec_type(),
            spec_draft_n_max: default_spec_draft_n_max(),
            spec_draft_n_min: 0,
            spec_draft_p_min: default_spec_draft_p_min(),
            spec_draft_p_split: default_spec_draft_p_split(),
            kv_offload: true,
            cache_type_k: "f16".to_string(),
            cache_type_v: "f16".to_string(),
            kv_mlock: false,
            kv_mmap: true,
            kv_unified: false,
            swa_full: false,
            kv_cache_ratio: default_kv_cache_ratio(),
            ctx_checkpoints: default_ctx_checkpoints(),
            checkpoint_min_step: default_checkpoint_min_step(),
            gpu_layers_mode: GpuLayersMode::Auto,
            split_mode: "none".to_string(),
            tensor_split: "".to_string(),
            cpu_moe: false,
            n_cpu_moe: 0,
            verbose: false,
            offline_mode: false,
            rpc_mode: false,
            rpc_endpoints: "127.0.0.1:50052".to_string(),
            web_ui_enabled: default_web_ui_enabled(),
            log_timestamps: default_log_timestamps(),
            session_timeout: default_session_timeout(),
        }
    }
}

impl Preset {
    /// 从当前 AppSettings 创建预设快照
    pub fn from_settings(settings: &AppSettings, name: String) -> Self {
        Self {
            name,
            host: settings.host.clone(),
            port: settings.port,
            parallel_slots: settings.parallel_slots,
            context: settings.context,
            batch_size: settings.batch_size,
            ubatch_size: settings.ubatch_size,
            temperature: settings.temperature,
            top_p: settings.top_p,
            top_k: settings.top_k,
            repeat_penalty: settings.repeat_penalty,
            presence_penalty: settings.presence_penalty,
            enable_temperature: settings.enable_temperature,
            enable_top_p: settings.enable_top_p,
            enable_top_k: settings.enable_top_k,
            enable_repeat_penalty: settings.enable_repeat_penalty,
            enable_presence_penalty: settings.enable_presence_penalty,
            flash_attn: settings.flash_attn.clone(),
            reasoning_mode: settings.reasoning_mode.clone(),
            reasoning_effort: settings.reasoning_effort.clone(),
            reasoning_format: settings.reasoning_format.clone(),
            reasoning_preserve: settings.reasoning_preserve.clone(),
            reasoning_budget: settings.reasoning_budget,
            reasoning_budget_message: settings.reasoning_budget_message.clone(),
            jinja_enabled: settings.jinja_enabled,
            chat_template_file: settings.chat_template_file.clone(),
            load_mode: settings.load_mode.clone(),
            spec_type: settings.spec_type.clone(),
            spec_draft_n_max: settings.spec_draft_n_max,
            spec_draft_n_min: settings.spec_draft_n_min,
            spec_draft_p_min: settings.spec_draft_p_min,
            spec_draft_p_split: settings.spec_draft_p_split,
            kv_offload: settings.kv_offload,
            cache_type_k: settings.cache_type_k.clone(),
            cache_type_v: settings.cache_type_v.clone(),
            kv_mlock: settings.kv_mlock,
            kv_mmap: settings.kv_mmap,
            kv_unified: settings.kv_unified,
            swa_full: settings.swa_full,
            kv_cache_ratio: settings.kv_cache_ratio,
            ctx_checkpoints: settings.ctx_checkpoints,
            checkpoint_min_step: settings.checkpoint_min_step,
            gpu_layers_mode: settings.gpu_layers_mode,
            split_mode: settings.split_mode.clone(),
            tensor_split: settings.tensor_split.clone(),
            cpu_moe: settings.cpu_moe,
            n_cpu_moe: settings.n_cpu_moe,
            verbose: settings.verbose,
            offline_mode: settings.offline_mode,
            rpc_mode: settings.rpc_mode,
            rpc_endpoints: settings.rpc_endpoints.clone(),
            web_ui_enabled: settings.web_ui_enabled,
            log_timestamps: settings.log_timestamps,
            session_timeout: settings.session_timeout,
        }
    }

    /// 将预设应用到 AppSettings
    pub fn apply_to(self, settings: &mut AppSettings) {
        settings.host = self.host;
        settings.port = self.port;
        settings.parallel_slots = self.parallel_slots;
        settings.context = self.context;
        settings.batch_size = self.batch_size;
        settings.ubatch_size = self.ubatch_size;
        settings.temperature = self.temperature;
        settings.top_p = self.top_p;
        settings.top_k = self.top_k;
        settings.repeat_penalty = self.repeat_penalty;
        settings.presence_penalty = self.presence_penalty;
        settings.enable_temperature = self.enable_temperature;
        settings.enable_top_p = self.enable_top_p;
        settings.enable_top_k = self.enable_top_k;
        settings.enable_repeat_penalty = self.enable_repeat_penalty;
        settings.enable_presence_penalty = self.enable_presence_penalty;
        settings.flash_attn = self.flash_attn;
        // 思考控制（Reasoning / Thinking）配置
        settings.reasoning_mode = self.reasoning_mode;
        settings.reasoning_effort = self.reasoning_effort;
        settings.reasoning_format = self.reasoning_format;
        settings.reasoning_preserve = self.reasoning_preserve;
        settings.reasoning_budget = self.reasoning_budget;
        settings.reasoning_budget_message = self.reasoning_budget_message;
        // 聊天模板
        settings.jinja_enabled = self.jinja_enabled;
        settings.chat_template_file = self.chat_template_file;
        // 加载模式
        settings.load_mode = self.load_mode;
        // 推测解码（Speculative Decoding）配置
        settings.spec_type = self.spec_type;
        settings.spec_draft_n_max = self.spec_draft_n_max;
        settings.spec_draft_n_min = self.spec_draft_n_min;
        settings.spec_draft_p_min = self.spec_draft_p_min;
        settings.spec_draft_p_split = self.spec_draft_p_split;
        settings.kv_offload = self.kv_offload;
        settings.cache_type_k = self.cache_type_k;
        settings.cache_type_v = self.cache_type_v;
        settings.kv_mlock = self.kv_mlock;
        settings.kv_mmap = self.kv_mmap;
        settings.kv_unified = self.kv_unified;
        settings.swa_full = self.swa_full;
        settings.kv_cache_ratio = self.kv_cache_ratio;
        settings.ctx_checkpoints = self.ctx_checkpoints;
        settings.checkpoint_min_step = self.checkpoint_min_step;
        settings.gpu_layers_mode = self.gpu_layers_mode;
        settings.split_mode = self.split_mode;
        settings.tensor_split = self.tensor_split;
        settings.cpu_moe = self.cpu_moe;
        settings.n_cpu_moe = self.n_cpu_moe;
        settings.verbose = self.verbose;
        settings.offline_mode = self.offline_mode;
        settings.rpc_mode = self.rpc_mode;
        settings.rpc_endpoints = self.rpc_endpoints;
        settings.web_ui_enabled = self.web_ui_enabled;
        settings.log_timestamps = self.log_timestamps;
        settings.session_timeout = self.session_timeout;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    // Server 配置
    pub server_path: PathBuf,
    pub host: String,
    pub port: u16,
    pub parallel_slots: usize,

    // 模型
    pub model_path: PathBuf,
    pub mmproj_path: PathBuf,
    #[serde(default)]
    pub dflash_path: PathBuf,
    #[serde(default)]
    pub model_dir: PathBuf,
    #[serde(default)]
    pub alias: String, // --alias

    // 推理参数（以 k 为单位存储，1k = 1024）
    #[serde(
        default = "default_context",
        deserialize_with = "deserialize_context::deserialize"
    )]
    pub context: usize, // --ctx-size (k)
    #[serde(
        default = "default_batch_size",
        deserialize_with = "deserialize_batch_size::deserialize"
    )]
    pub batch_size: usize, // --batch-size (k)
    #[serde(
        default = "default_ubatch_size",
        deserialize_with = "deserialize_ubatch_size::deserialize"
    )]
    pub ubatch_size: f32, // --ubatch-size (k, 0.5 步进)
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: i32,
    pub repeat_penalty: f32,
    pub presence_penalty: f32,
    // 采样参数启用标志（勾选时拼接启动命令）
    #[serde(default)]
    pub enable_temperature: bool,
    #[serde(default)]
    pub enable_top_p: bool,
    #[serde(default)]
    pub enable_top_k: bool,
    #[serde(default)]
    pub enable_repeat_penalty: bool,
    #[serde(default)]
    pub enable_presence_penalty: bool,
    #[serde(default = "default_flash_attn")]
    pub flash_attn: String,

    // 思考控制（Reasoning / Thinking）配置
    #[serde(default = "default_reasoning_mode")]
    pub reasoning_mode: String, // --reasoning (auto/on/off)
    #[serde(default = "default_reasoning_effort")]
    pub reasoning_effort: String, // --reasoning-effort ("default" 时不拼接)
    #[serde(default = "default_reasoning_format")]
    pub reasoning_format: String, // --reasoning-format (auto/none/deepseek/deepseek-legacy)
    #[serde(default)]
    pub reasoning_preserve: String, // "" / "on" / "off" → 不传 / --reasoning-preserve / --no-reasoning-preserve
    #[serde(default = "default_reasoning_budget")]
    pub reasoning_budget: i64, // --reasoning-budget (-1 不拼接)
    #[serde(default)]
    pub reasoning_budget_message: String, // --reasoning-budget-message (空不拼接)

    // 聊天模板
    #[serde(default = "default_jinja_enabled")]
    pub jinja_enabled: bool, // --jinja / --no-jinja
    #[serde(default)]
    pub chat_template_file: PathBuf, // --chat-template-file (空不拼接)

    // 加载模式（新版 --load-mode；"auto" 时沿用旧版 --mmap/--mlock 行为）
    #[serde(default = "default_load_mode")]
    pub load_mode: String, // --load-mode

    // 推测解码（Speculative Decoding）配置
    #[serde(default = "default_spec_type")]
    pub spec_type: String, // --spec-type
    #[serde(default = "default_spec_draft_n_max")]
    pub spec_draft_n_max: usize, // --spec-draft-n-max
    #[serde(default)]
    pub spec_draft_n_min: usize, // --spec-draft-n-min
    #[serde(default = "default_spec_draft_p_min")]
    pub spec_draft_p_min: f32, // --spec-draft-p-min
    #[serde(default = "default_spec_draft_p_split")]
    pub spec_draft_p_split: f32, // --spec-draft-p-split

    // KV 缓存配置
    pub kv_offload: bool,
    pub cache_type_k: String,
    pub cache_type_v: String,
    pub kv_mlock: bool,   // --mlock
    pub kv_mmap: bool,    // --mmap / --no-mmap
    pub kv_unified: bool, // --kv-unified
    #[serde(default)]
    pub swa_full: bool, // --swa-full
    #[serde(default = "default_kv_cache_ratio")]
    pub kv_cache_ratio: f32, // KV 缓存比例 (不拼接启动命令)
    #[serde(default = "default_ctx_checkpoints")]
    pub ctx_checkpoints: usize, // --ctx-checkpoints
    #[serde(default = "default_checkpoint_min_step")]
    pub checkpoint_min_step: usize, // --checkpoint-min-step

    // GPU 与设备分配
    pub gpu_layers_mode: GpuLayersMode,
    pub split_mode: String,
    pub tensor_split: String,
    pub cpu_moe: bool,
    pub n_cpu_moe: usize,

    // RPC 配置
    pub rpc_server_path: PathBuf,
    pub rpc_host: String,
    pub rpc_port: u16,
    pub rpc_threads: usize,
    pub rpc_device: String,
    pub rpc_cache: bool,

    // 高级
    pub verbose: bool,

    // 离线模式
    #[serde(default)]
    pub offline_mode: bool,

    // RPC 模式 (llama-server)
    #[serde(default)]
    pub rpc_mode: bool,
    #[serde(default)]
    pub rpc_endpoints: String,

    // 网页客户端开关（默认启用）
    #[serde(default = "default_web_ui_enabled")]
    pub web_ui_enabled: bool,

    // 原生日志时间戳（默认启用）
    #[serde(default = "default_log_timestamps")]
    pub log_timestamps: bool,

    // 会话超时（秒，追加 --timeout）
    #[serde(default = "default_session_timeout")]
    pub session_timeout: usize,

    // 预设
    #[serde(default)]
    pub presets: Vec<Preset>,

    // 预设 UI 状态（不序列化）
    #[serde(skip, default)]
    pub new_preset_name: String,
    #[serde(skip, default)]
    pub rename_preset_index: Option<usize>,
    #[serde(skip, default)]
    pub rename_preset_new_name: String,

    // 自启动预设名称
    #[serde(default)]
    pub auto_start_preset_name: Option<String>,

    // 日志面板设置
    #[serde(default = "default_auto_scroll_logs")]
    pub auto_scroll_logs: bool,
    #[serde(default = "default_max_log_lines")]
    pub max_log_lines: i32,

    // 开机自启动
    #[serde(default)]
    pub auto_start: bool,

    // 文件日志开关（默认开启）
    #[serde(default = "default_log_to_file")]
    pub log_to_file: bool,

    // 界面主题：深色模式（默认开启）
    #[serde(default = "default_dark_mode")]
    pub dark_mode: bool,

    // 界面主题模式："light" / "dark" / "auto"
    #[serde(default = "default_theme_mode")]
    pub theme_mode: String,

    // 界面主题色（十六进制，如 #0A84FF），全局强调色
    #[serde(default = "default_accent_color")]
    pub accent_color: String,

    // 界面语言（"zh" / "en"），空字符串表示首次运行按系统区域检测
    #[serde(default)]
    pub language: String,

    // llama.cpp 版本信息（不序列化，运行时缓存）
    #[serde(skip, default)]
    pub llama_version: String,

    // KV 缓存计算结果（运行时缓存，不序列化）
    #[serde(skip, default)]
    pub kv_cache_result: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            server_path: PathBuf::new(),
            host: "127.0.0.1".to_string(),
            port: 8080,
            parallel_slots: 1,
            model_path: PathBuf::new(),
            mmproj_path: PathBuf::new(),
            dflash_path: PathBuf::new(),
            model_dir: PathBuf::new(),
            alias: String::new(),
            context: 4,       // 4k = 4096
            batch_size: 2,    // 2k = 2048
            ubatch_size: 0.5, // 0.5k = 512
            temperature: 0.8,
            top_p: 0.95,
            top_k: 40,
            repeat_penalty: 1.1,
            presence_penalty: 0.0,
            enable_temperature: false,
            enable_top_p: false,
            enable_top_k: false,
            enable_repeat_penalty: false,
            enable_presence_penalty: false,
            flash_attn: default_flash_attn(),
            reasoning_mode: default_reasoning_mode(),
            reasoning_effort: default_reasoning_effort(),
            reasoning_format: default_reasoning_format(),
            reasoning_preserve: String::new(),
            reasoning_budget: default_reasoning_budget(),
            reasoning_budget_message: String::new(),
            jinja_enabled: default_jinja_enabled(),
            chat_template_file: PathBuf::new(),
            load_mode: default_load_mode(),
            spec_type: default_spec_type(),
            spec_draft_n_max: default_spec_draft_n_max(),
            spec_draft_n_min: 0,
            spec_draft_p_min: default_spec_draft_p_min(),
            spec_draft_p_split: default_spec_draft_p_split(),
            kv_offload: true,
            cache_type_k: "f16".to_string(),
            cache_type_v: "f16".to_string(),
            kv_mlock: false,
            kv_mmap: true,
            kv_unified: false,
            swa_full: false,
            kv_cache_ratio: default_kv_cache_ratio(),
            ctx_checkpoints: default_ctx_checkpoints(),
            checkpoint_min_step: default_checkpoint_min_step(),
            gpu_layers_mode: GpuLayersMode::Auto,
            split_mode: "none".to_string(),
            tensor_split: "".to_string(),
            cpu_moe: false,
            n_cpu_moe: 0,
            rpc_server_path: PathBuf::new(),
            rpc_host: "127.0.0.1".to_string(),
            rpc_port: 50052,
            rpc_threads: 8,
            rpc_device: "".to_string(),
            rpc_cache: false,
            verbose: false,
            offline_mode: false,
            rpc_mode: false,
            rpc_endpoints: "127.0.0.1:50052".to_string(),
            web_ui_enabled: default_web_ui_enabled(),
            log_timestamps: default_log_timestamps(),
            session_timeout: default_session_timeout(),
            presets: Vec::new(),
            new_preset_name: String::new(),
            rename_preset_index: None,
            rename_preset_new_name: String::new(),
            auto_scroll_logs: default_auto_scroll_logs(),
            max_log_lines: default_max_log_lines(),
            auto_start: false,
            log_to_file: default_log_to_file(),
            dark_mode: true,
            theme_mode: "auto".to_string(),
            accent_color: default_accent_color(),
            language: String::new(),
            auto_start_preset_name: None,
            llama_version: String::new(),
            kv_cache_result: None,
        }
    }
}

impl AppSettings {
    /// k 值 → 实际参数值 (value * 1024)
    pub fn context_actual(&self) -> usize {
        self.context * 1024
    }
    pub fn batch_size_actual(&self) -> usize {
        self.batch_size * 1024
    }
    pub fn ubatch_size_actual(&self) -> usize {
        (self.ubatch_size * 1024.0) as usize
    }
}

pub struct SettingsManager {
    config_dir: PathBuf,
}

impl SettingsManager {
    pub fn new() -> Self {
        let config_dir = std::env::current_exe()
            .map(|p| p.parent().unwrap_or(Path::new("")).to_path_buf())
            .unwrap_or_else(|_| PathBuf::from("."));

        Self { config_dir }
    }

    pub fn load(&self) -> Result<AppSettings, String> {
        let path = self.config_dir.join(CONFIG_FILE);
        if !path.exists() {
            return Ok(AppSettings::default());
        }
        let content = fs::read_to_string(&path).map_err(|e| format!("读取配置失败: {}", e))?;
        serde_json::from_str(&content).map_err(|e| format!("解析配置失败: {}", e))
    }

    pub fn save(&self, settings: &AppSettings) -> Result<(), String> {
        let path = self.config_dir.join(CONFIG_FILE);
        let content =
            serde_json::to_string_pretty(settings).map_err(|e| format!("序列化配置失败: {}", e))?;
        fs::write(&path, content).map_err(|e| format!("写入配置失败: {}", e))?;
        Ok(())
    }

    /// 在指定目录中查找指定名称的可执行文件
    fn find_exe_in_dir(dir: &Path, name: &str) -> Option<PathBuf> {
        let filename = if cfg!(target_os = "windows") {
            format!("{}.exe", name)
        } else {
            name.to_string()
        };
        let path = dir.join(&filename);
        if path.exists() {
            Some(path)
        } else {
            None
        }
    }

    /// 在指定目录及其匹配关键词的子目录中查找可执行文件
    /// 优先级：1. 目录本身  2. 名称包含 keyword 的子目录（按目录名排序，保证确定性）
    fn find_exe_recursive(&self, dir: &Path, exe_name: &str, keyword: &str) -> Option<PathBuf> {
        // 1. 先在目录本身查找
        if let Some(path) = Self::find_exe_in_dir(dir, exe_name) {
            return Some(path);
        }

        // 2. 在名称包含 keyword 的子目录中查找
        let entries = fs::read_dir(dir).ok()?;
        let mut subdirs: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .to_lowercase()
                    .contains(&keyword.to_lowercase())
            })
            .map(|e| e.path())
            .collect();
        subdirs.sort();

        for subdir in subdirs {
            if let Some(path) = Self::find_exe_in_dir(&subdir, exe_name) {
                return Some(path);
            }
        }

        None
    }

    /// 自动检测 llama-server 路径
    /// 搜索：exe 同级目录 → 含 "llama" 名称的子目录
    pub fn auto_detect_server_path(&self) -> Option<PathBuf> {
        self.find_exe_recursive(&self.config_dir, "llama-server", "llama")
    }

    /// 自动检测 rpc-server 路径
    /// 搜索：exe 同级目录 → 含 "llama" 名称的子目录（通常与 llama-server 同目录）
    pub fn auto_detect_rpc_path(&self) -> Option<PathBuf> {
        self.find_exe_recursive(&self.config_dir, "ggml-rpc-server", "llama")
    }
}

/// 判断文件名是否为 llama-server 可执行文件（跨平台）
pub fn is_server_binary_name(name: &str) -> bool {
    if cfg!(target_os = "windows") {
        name == "llama-server.exe"
    } else {
        name == "llama-server"
    }
}

/// 判断文件名是否为 rpc-server（ggml-rpc-server）可执行文件（跨平台）
pub fn is_rpc_binary_name(name: &str) -> bool {
    if cfg!(target_os = "windows") {
        name == "ggml-rpc-server.exe" || name == "rpc-server.exe"
    } else {
        name == "ggml-rpc-server" || name == "rpc-server"
    }
}
