use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
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

fn default_load_mode() -> String {
    "auto".to_string() // --load-mode，"auto" = 不拼接并沿用旧版 --mmap/--mlock
}

// ── 新增参数（llama.cpp b10488+）默认值 ──

// 线程与生成长度
fn default_threads() -> i64 {
    -1 // --threads，-1 = 不拼接（沿用 llama 默认）
}

fn default_threads_batch() -> i64 {
    -1 // --threads-batch，-1 = 不拼接（沿用 llama 默认）
}

fn default_n_predict() -> i64 {
    -1 // --n-predict，-1 = 不拼接（无限生成）
}

fn default_keep_tokens() -> i64 {
    0 // --keep，0 = 不拼接
}

// 随机种子
fn default_seed() -> i64 {
    -1 // --seed，-1 = 不拼接（随机种子）
}

// 主 GPU（多卡时指定）
fn default_main_gpu() -> i64 {
    0 // --main-gpu，默认第一张卡；>=0 时拼接
}

// 采样器扩展
fn default_min_p() -> f32 {
    0.05 // --min-p，默认与 llama 一致
}

fn default_top_n_sigma() -> f32 {
    -1.0 // --top-n-sigma，-1 = 禁用（不拼接）
}

fn default_xtc_probability() -> f32 {
    0.0 // --xtc-probability，0 = 禁用（不拼接）
}

fn default_xtc_threshold() -> f32 {
    0.10
}

fn default_typical_p() -> f32 {
    1.0 // --typical-p，1.0 = 禁用（不拼接）
}

fn default_mirostat() -> i32 {
    0 // --mirostat，0 = 禁用
}

fn default_mirostat_lr() -> f32 {
    0.10
}

fn default_mirostat_ent() -> f32 {
    5.00
}

fn default_dynatemp_range() -> f32 {
    0.0 // --dynatemp-range，0 = 禁用（不拼接）
}

fn default_dynatemp_exp() -> f32 {
    1.0
}

fn default_sampler_seq() -> String {
    "".to_string() // --sampler-seq，空 = 不拼接（沿用 llama 默认 edskypmxt）
}

// 长上下文
fn default_cache_prompt() -> bool {
    true // --cache-prompt / --no-cache-prompt（llama 默认启用）
}

fn default_cache_reuse() -> i64 {
    0 // --cache-reuse，0 = 不拼接
}

fn default_context_shift() -> bool {
    false // --context-shift（llama 默认禁用）
}

// JSON/语法约束
fn default_json_schema() -> String {
    "".to_string() // --json-schema / --json-schema-file
}

fn default_grammar() -> String {
    "".to_string() // --grammar（空 = 不拼接）
}

fn default_api_key() -> String {
    "".to_string() // --api-key（空 = 不拼接）
}

fn default_api_prefix() -> String {
    "".to_string() // --api-prefix（空 = 不拼接）
}

fn default_cors_origins() -> String {
    "".to_string() // --cors-origins（空 = 不拼接）
}

fn default_numa() -> String {
    "".to_string() // --numa（空 = 不拼接）
}

fn default_web_ui_enabled() -> bool {
    true
}

fn default_log_timestamps() -> bool {
    true
}

fn default_log_verbosity() -> usize {
    3 // --log-verbosity 默认 info
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

// llama.cpp 下载变体偏好默认值
fn default_download_variant() -> String {
    "cpu".to_string()
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

/// 值直接以 k 为单位存储，最小值为 1
fn from_raw_or_k(v: usize) -> usize {
    v.max(1)
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

// 思考与会话默认值
fn default_reasoning() -> String {
    "auto".to_string() // --reasoning: auto/on/off
}

fn default_reasoning_format() -> String {
    "auto".to_string() // --reasoning-format: auto/none/deepseek/deepseek-legacy
}

fn default_reasoning_effort() -> String {
    "default".to_string() // --chat-template-kwargs: JSON 值（如 {"reasoning_effort": "high"}）；default = 不拼接
}

fn default_reasoning_budget() -> i32 {
    -1 // --reasoning-budget: -1 = 不限制
}

fn default_jinja_enabled() -> bool {
    true // --jinja / --no-jinja（新版 llama-server 默认启用）
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

    // 加载模式（新版 --load-mode；"auto" 时沿用旧版 --mmap/--mlock 行为）
    #[serde(default = "default_load_mode")]
    pub load_mode: String, // --load-mode

    // ── 线程与生成长度（llama.cpp b10488+）──
    #[serde(default = "default_threads")]
    pub threads: i64, // --threads (-1 不拼接)
    #[serde(default = "default_threads_batch")]
    pub threads_batch: i64, // --threads-batch (-1 不拼接)
    #[serde(default = "default_n_predict")]
    pub n_predict: i64, // --n-predict (-1 不拼接)
    #[serde(default = "default_keep_tokens")]
    pub keep: i64, // --keep (0 不拼接)
    #[serde(default = "default_seed")]
    pub seed: i64, // --seed (-1 不拼接)
    #[serde(default = "default_main_gpu")]
    pub main_gpu: i64, // --main-gpu

    // ── 采样器扩展（近新版本加入）──
    #[serde(default)]
    pub enable_min_p: bool,
    #[serde(default = "default_min_p")]
    pub min_p: f32, // --min-p
    #[serde(default)]
    pub enable_top_n_sigma: bool,
    #[serde(default = "default_top_n_sigma")]
    pub top_n_sigma: f32, // --top-n-sigma
    #[serde(default)]
    pub enable_xtc: bool,
    #[serde(default = "default_xtc_probability")]
    pub xtc_probability: f32, // --xtc-probability
    #[serde(default = "default_xtc_threshold")]
    pub xtc_threshold: f32, // --xtc-threshold
    #[serde(default)]
    pub enable_typical_p: bool,
    #[serde(default = "default_typical_p")]
    pub typical_p: f32, // --typical-p
    #[serde(default)]
    pub enable_mirostat: bool,
    #[serde(default = "default_mirostat")]
    pub mirostat: i32, // --mirostat (0/1/2)
    #[serde(default = "default_mirostat_lr")]
    pub mirostat_lr: f32, // --mirostat-lr
    #[serde(default = "default_mirostat_ent")]
    pub mirostat_ent: f32, // --mirostat-ent
    #[serde(default)]
    pub enable_dynatemp: bool,
    #[serde(default = "default_dynatemp_range")]
    pub dynatemp_range: f32, // --dynatemp-range
    #[serde(default = "default_dynatemp_exp")]
    pub dynatemp_exp: f32, // --dynatemp-exp
    #[serde(default = "default_sampler_seq")]
    pub sampler_seq: String, // --sampler-seq (空不拼接)

    // ── 长上下文 / 提示缓存 ──
    #[serde(default = "default_cache_prompt")]
    pub cache_prompt: bool, // --cache-prompt / --no-cache-prompt
    #[serde(default = "default_cache_reuse")]
    pub cache_reuse: i64, // --cache-reuse (0 不拼接)
    #[serde(default = "default_context_shift")]
    pub context_shift: bool, // --context-shift

    // ── 结构化输出 ──
    #[serde(default = "default_json_schema")]
    pub json_schema: String, // --json-schema (空不拼接)
    #[serde(default = "default_grammar")]
    pub grammar: String, // --grammar (空不拼接)

    // ── API 安全 / 部署 ──
    #[serde(default = "default_api_key")]
    pub api_key: String, // --api-key (空不拼接)
    #[serde(default = "default_api_prefix")]
    pub api_prefix: String, // --api-prefix (空不拼接)
    #[serde(default = "default_cors_origins")]
    pub cors_origins: String, // --cors-origins (空不拼接)
    #[serde(default)]
    pub ssl_cert_file: PathBuf, // --ssl-cert-file (空不拼接)
    #[serde(default)]
    pub ssl_key_file: PathBuf, // --ssl-key-file (空不拼接)
    #[serde(default)]
    pub reuse_port: bool, // --reuse-port
    #[serde(default = "default_numa")]
    pub numa: String, // --numa (空不拼接)

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
    #[serde(default)]
    pub override_tensor: String, // --override-tensor
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

    // 日志级别（0=generic 1=error 2=warning 3=info 4=trace 5=debug）
    #[serde(default = "default_log_verbosity")]
    pub log_verbosity: usize,

    // 会话超时（秒）
    #[serde(default = "default_session_timeout")]
    pub session_timeout: usize,

    // 思考与会话
    #[serde(default = "default_reasoning")]
    pub reasoning: String, // --reasoning: auto/on/off
    #[serde(default = "default_reasoning_format")]
    pub reasoning_format: String, // --reasoning-format
    #[serde(default = "default_reasoning_effort")]
    pub reasoning_effort: String, // --chat-template-kwargs：JSON 值（如 {"reasoning_effort": "high"}）
    #[serde(default = "default_reasoning_budget")]
    pub reasoning_budget: i32, // --reasoning-budget: -1 = 不限制
    #[serde(default)]
    pub reasoning_preserve: Option<bool>, // --reasoning-preserve / --no-reasoning-preserve：None = 模型默认（不拼接）
    #[serde(default = "default_jinja_enabled")]
    pub jinja_enabled: bool, // --jinja / --no-jinja
    #[serde(default)]
    pub chat_template: String, // --chat-template（Jinja 模板文本）
    #[serde(default)]
    pub chat_template_file: PathBuf, // --chat-template-file（Jinja 模板文件）
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
            load_mode: default_load_mode(),
            threads: default_threads(),
            threads_batch: default_threads_batch(),
            n_predict: default_n_predict(),
            keep: default_keep_tokens(),
            seed: default_seed(),
            main_gpu: default_main_gpu(),
            enable_min_p: false,
            min_p: default_min_p(),
            enable_top_n_sigma: false,
            top_n_sigma: default_top_n_sigma(),
            enable_xtc: false,
            xtc_probability: default_xtc_probability(),
            xtc_threshold: default_xtc_threshold(),
            enable_typical_p: false,
            typical_p: default_typical_p(),
            enable_mirostat: false,
            mirostat: default_mirostat(),
            mirostat_lr: default_mirostat_lr(),
            mirostat_ent: default_mirostat_ent(),
            enable_dynatemp: false,
            dynatemp_range: default_dynatemp_range(),
            dynatemp_exp: default_dynatemp_exp(),
            sampler_seq: default_sampler_seq(),
            cache_prompt: default_cache_prompt(),
            cache_reuse: default_cache_reuse(),
            context_shift: default_context_shift(),
            json_schema: default_json_schema(),
            grammar: default_grammar(),
            api_key: default_api_key(),
            api_prefix: default_api_prefix(),
            cors_origins: default_cors_origins(),
            ssl_cert_file: PathBuf::new(),
            ssl_key_file: PathBuf::new(),
            reuse_port: false,
            numa: default_numa(),
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
            override_tensor: String::new(),
            verbose: false,
            offline_mode: false,
            rpc_mode: false,
            rpc_endpoints: "127.0.0.1:50052".to_string(),
            web_ui_enabled: default_web_ui_enabled(),
            log_timestamps: default_log_timestamps(),
            log_verbosity: default_log_verbosity(),
            session_timeout: default_session_timeout(),
            reasoning: default_reasoning(),
            reasoning_format: default_reasoning_format(),
            reasoning_effort: default_reasoning_effort(),
            reasoning_budget: default_reasoning_budget(),
            reasoning_preserve: None,
            jinja_enabled: default_jinja_enabled(),
            chat_template: String::new(),
            chat_template_file: PathBuf::new(),
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
            load_mode: settings.load_mode.clone(),
            threads: settings.threads,
            threads_batch: settings.threads_batch,
            n_predict: settings.n_predict,
            keep: settings.keep,
            seed: settings.seed,
            main_gpu: settings.main_gpu,
            enable_min_p: settings.enable_min_p,
            min_p: settings.min_p,
            enable_top_n_sigma: settings.enable_top_n_sigma,
            top_n_sigma: settings.top_n_sigma,
            enable_xtc: settings.enable_xtc,
            xtc_probability: settings.xtc_probability,
            xtc_threshold: settings.xtc_threshold,
            enable_typical_p: settings.enable_typical_p,
            typical_p: settings.typical_p,
            enable_mirostat: settings.enable_mirostat,
            mirostat: settings.mirostat,
            mirostat_lr: settings.mirostat_lr,
            mirostat_ent: settings.mirostat_ent,
            enable_dynatemp: settings.enable_dynatemp,
            dynatemp_range: settings.dynatemp_range,
            dynatemp_exp: settings.dynatemp_exp,
            sampler_seq: settings.sampler_seq.clone(),
            cache_prompt: settings.cache_prompt,
            cache_reuse: settings.cache_reuse,
            context_shift: settings.context_shift,
            json_schema: settings.json_schema.clone(),
            grammar: settings.grammar.clone(),
            api_key: settings.api_key.clone(),
            api_prefix: settings.api_prefix.clone(),
            cors_origins: settings.cors_origins.clone(),
            ssl_cert_file: settings.ssl_cert_file.clone(),
            ssl_key_file: settings.ssl_key_file.clone(),
            reuse_port: settings.reuse_port,
            numa: settings.numa.clone(),
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
            override_tensor: settings.override_tensor.clone(),
            verbose: settings.verbose,
            offline_mode: settings.offline_mode,
            rpc_mode: settings.rpc_mode,
            rpc_endpoints: settings.rpc_endpoints.clone(),
            web_ui_enabled: settings.web_ui_enabled,
            log_timestamps: settings.log_timestamps,
            log_verbosity: settings.log_verbosity,
            session_timeout: settings.session_timeout,
            reasoning: settings.reasoning.clone(),
            reasoning_format: settings.reasoning_format.clone(),
            reasoning_effort: settings.reasoning_effort.clone(),
            reasoning_budget: settings.reasoning_budget,
            reasoning_preserve: settings.reasoning_preserve,
            jinja_enabled: settings.jinja_enabled,
            chat_template: settings.chat_template.clone(),
            chat_template_file: settings.chat_template_file.clone(),
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
        // 加载模式
        settings.load_mode = self.load_mode;
        // 线程与生成长度
        settings.threads = self.threads;
        settings.threads_batch = self.threads_batch;
        settings.n_predict = self.n_predict;
        settings.keep = self.keep;
        settings.seed = self.seed;
        settings.main_gpu = self.main_gpu;
        // 采样器扩展
        settings.enable_min_p = self.enable_min_p;
        settings.min_p = self.min_p;
        settings.enable_top_n_sigma = self.enable_top_n_sigma;
        settings.top_n_sigma = self.top_n_sigma;
        settings.enable_xtc = self.enable_xtc;
        settings.xtc_probability = self.xtc_probability;
        settings.xtc_threshold = self.xtc_threshold;
        settings.enable_typical_p = self.enable_typical_p;
        settings.typical_p = self.typical_p;
        settings.enable_mirostat = self.enable_mirostat;
        settings.mirostat = self.mirostat;
        settings.mirostat_lr = self.mirostat_lr;
        settings.mirostat_ent = self.mirostat_ent;
        settings.enable_dynatemp = self.enable_dynatemp;
        settings.dynatemp_range = self.dynatemp_range;
        settings.dynatemp_exp = self.dynatemp_exp;
        settings.sampler_seq = self.sampler_seq;
        // 长上下文 / 提示缓存
        settings.cache_prompt = self.cache_prompt;
        settings.cache_reuse = self.cache_reuse;
        settings.context_shift = self.context_shift;
        // 结构化输出
        settings.json_schema = self.json_schema;
        settings.grammar = self.grammar;
        // API 安全 / 部署
        settings.api_key = self.api_key;
        settings.api_prefix = self.api_prefix;
        settings.cors_origins = self.cors_origins;
        settings.ssl_cert_file = self.ssl_cert_file;
        settings.ssl_key_file = self.ssl_key_file;
        settings.reuse_port = self.reuse_port;
        settings.numa = self.numa;
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
        settings.override_tensor = self.override_tensor;
        settings.verbose = self.verbose;
        settings.offline_mode = self.offline_mode;
        settings.rpc_mode = self.rpc_mode;
        settings.rpc_endpoints = self.rpc_endpoints;
        settings.web_ui_enabled = self.web_ui_enabled;
        settings.log_timestamps = self.log_timestamps;
        settings.log_verbosity = self.log_verbosity;
        settings.session_timeout = self.session_timeout;
        // 思考与会话
        settings.reasoning = self.reasoning;
        settings.reasoning_format = self.reasoning_format;
        settings.reasoning_effort = self.reasoning_effort;
        settings.reasoning_budget = self.reasoning_budget;
        settings.reasoning_preserve = self.reasoning_preserve;
        settings.jinja_enabled = self.jinja_enabled;
        settings.chat_template = self.chat_template;
        settings.chat_template_file = self.chat_template_file;
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

    // 加载模式（新版 --load-mode；"auto" 时沿用旧版 --mmap/--mlock 行为）
    #[serde(default = "default_load_mode")]
    pub load_mode: String, // --load-mode

    // ── 线程与生成长度（llama.cpp b10488+）──
    #[serde(default = "default_threads")]
    pub threads: i64, // --threads (-1 不拼接)
    #[serde(default = "default_threads_batch")]
    pub threads_batch: i64, // --threads-batch (-1 不拼接)
    #[serde(default = "default_n_predict")]
    pub n_predict: i64, // --n-predict (-1 不拼接)
    #[serde(default = "default_keep_tokens")]
    pub keep: i64, // --keep (0 不拼接)
    #[serde(default = "default_seed")]
    pub seed: i64, // --seed (-1 不拼接)
    #[serde(default = "default_main_gpu")]
    pub main_gpu: i64, // --main-gpu

    // ── 采样器扩展（近新版本加入）──
    #[serde(default)]
    pub enable_min_p: bool,
    #[serde(default = "default_min_p")]
    pub min_p: f32, // --min-p
    #[serde(default)]
    pub enable_top_n_sigma: bool,
    #[serde(default = "default_top_n_sigma")]
    pub top_n_sigma: f32, // --top-n-sigma
    #[serde(default)]
    pub enable_xtc: bool,
    #[serde(default = "default_xtc_probability")]
    pub xtc_probability: f32, // --xtc-probability
    #[serde(default = "default_xtc_threshold")]
    pub xtc_threshold: f32, // --xtc-threshold
    #[serde(default)]
    pub enable_typical_p: bool,
    #[serde(default = "default_typical_p")]
    pub typical_p: f32, // --typical-p
    #[serde(default)]
    pub enable_mirostat: bool,
    #[serde(default = "default_mirostat")]
    pub mirostat: i32, // --mirostat (0/1/2)
    #[serde(default = "default_mirostat_lr")]
    pub mirostat_lr: f32, // --mirostat-lr
    #[serde(default = "default_mirostat_ent")]
    pub mirostat_ent: f32, // --mirostat-ent
    #[serde(default)]
    pub enable_dynatemp: bool,
    #[serde(default = "default_dynatemp_range")]
    pub dynatemp_range: f32, // --dynatemp-range
    #[serde(default = "default_dynatemp_exp")]
    pub dynatemp_exp: f32, // --dynatemp-exp
    #[serde(default = "default_sampler_seq")]
    pub sampler_seq: String, // --sampler-seq (空不拼接)

    // ── 长上下文 / 提示缓存 ──
    #[serde(default = "default_cache_prompt")]
    pub cache_prompt: bool, // --cache-prompt / --no-cache-prompt
    #[serde(default = "default_cache_reuse")]
    pub cache_reuse: i64, // --cache-reuse (0 不拼接)
    #[serde(default = "default_context_shift")]
    pub context_shift: bool, // --context-shift

    // ── 结构化输出 ──
    #[serde(default = "default_json_schema")]
    pub json_schema: String, // --json-schema (空不拼接)
    #[serde(default = "default_grammar")]
    pub grammar: String, // --grammar (空不拼接)

    // ── API 安全 / 部署 ──
    #[serde(default = "default_api_key")]
    pub api_key: String, // --api-key (空不拼接)
    #[serde(default = "default_api_prefix")]
    pub api_prefix: String, // --api-prefix (空不拼接)
    #[serde(default = "default_cors_origins")]
    pub cors_origins: String, // --cors-origins (空不拼接)
    #[serde(default)]
    pub ssl_cert_file: PathBuf, // --ssl-cert-file (空不拼接)
    #[serde(default)]
    pub ssl_key_file: PathBuf, // --ssl-key-file (空不拼接)
    #[serde(default)]
    pub reuse_port: bool, // --reuse-port
    #[serde(default = "default_numa")]
    pub numa: String, // --numa (空不拼接)

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
    #[serde(default)]
    pub override_tensor: String, // --override-tensor

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

    // 日志级别（--log-verbosity，0=generic 1=error 2=warning 3=info 4=trace 5=debug）
    #[serde(default = "default_log_verbosity")]
    pub log_verbosity: usize,

    // 会话超时（秒，追加 --timeout）
    #[serde(default = "default_session_timeout")]
    pub session_timeout: usize,

    // 思考与会话
    #[serde(default = "default_reasoning")]
    pub reasoning: String, // --reasoning: auto/on/off
    #[serde(default = "default_reasoning_format")]
    pub reasoning_format: String, // --reasoning-format
    #[serde(default = "default_reasoning_effort")]
    pub reasoning_effort: String, // --chat-template-kwargs：JSON 值（如 {"reasoning_effort": "high"}）
    #[serde(default = "default_reasoning_budget")]
    pub reasoning_budget: i32, // --reasoning-budget: -1 = 不限制
    #[serde(default)]
    pub reasoning_preserve: Option<bool>, // --reasoning-preserve / --no-reasoning-preserve：None = 模型默认（不拼接）
    #[serde(default = "default_jinja_enabled")]
    pub jinja_enabled: bool, // --jinja / --no-jinja
    #[serde(default)]
    pub chat_template: String, // --chat-template（Jinja 模板文本）
    #[serde(default)]
    pub chat_template_file: PathBuf, // --chat-template-file（Jinja 模板文件）

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

    // 静默启动（开机自启时最小化到任务栏）
    #[serde(default)]
    pub silent_start: bool,

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

    // llama.cpp 下载变体偏好：
    // "cpu" | "cuda124" | "cuda133" | "rocm714" | "vulkan"
    // （兼容旧值 "gpu"：Windows→cuda124, Linux→vulkan）
    #[serde(default = "default_download_variant")]
    pub download_variant: String,

    // llama.cpp 版本信息（不序列化，运行时缓存）
    #[serde(skip, default)]
    pub llama_version: String,

    // 检查更新结果（不序列化，运行时缓存）
    // Some(true)=有新版本 Some(false)=已是最新 None=尚未检查
    #[serde(skip, default)]
    pub update_available: Option<bool>,

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
            load_mode: default_load_mode(),
            threads: default_threads(),
            threads_batch: default_threads_batch(),
            n_predict: default_n_predict(),
            keep: default_keep_tokens(),
            seed: default_seed(),
            main_gpu: default_main_gpu(),
            enable_min_p: false,
            min_p: default_min_p(),
            enable_top_n_sigma: false,
            top_n_sigma: default_top_n_sigma(),
            enable_xtc: false,
            xtc_probability: default_xtc_probability(),
            xtc_threshold: default_xtc_threshold(),
            enable_typical_p: false,
            typical_p: default_typical_p(),
            enable_mirostat: false,
            mirostat: default_mirostat(),
            mirostat_lr: default_mirostat_lr(),
            mirostat_ent: default_mirostat_ent(),
            enable_dynatemp: false,
            dynatemp_range: default_dynatemp_range(),
            dynatemp_exp: default_dynatemp_exp(),
            sampler_seq: default_sampler_seq(),
            cache_prompt: default_cache_prompt(),
            cache_reuse: default_cache_reuse(),
            context_shift: default_context_shift(),
            json_schema: default_json_schema(),
            grammar: default_grammar(),
            api_key: default_api_key(),
            api_prefix: default_api_prefix(),
            cors_origins: default_cors_origins(),
            ssl_cert_file: PathBuf::new(),
            ssl_key_file: PathBuf::new(),
            reuse_port: false,
            numa: default_numa(),
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
            override_tensor: String::new(),
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
            log_verbosity: default_log_verbosity(),
            session_timeout: default_session_timeout(),
            reasoning: default_reasoning(),
            reasoning_format: default_reasoning_format(),
            reasoning_effort: default_reasoning_effort(),
            reasoning_budget: default_reasoning_budget(),
            reasoning_preserve: None,
            jinja_enabled: default_jinja_enabled(),
            chat_template: String::new(),
            chat_template_file: PathBuf::new(),
            presets: Vec::new(),
            new_preset_name: String::new(),
            rename_preset_index: None,
            rename_preset_new_name: String::new(),
            auto_scroll_logs: default_auto_scroll_logs(),
            max_log_lines: default_max_log_lines(),
            auto_start: false,
            silent_start: false,
            log_to_file: default_log_to_file(),
            dark_mode: true,
            theme_mode: "auto".to_string(),
            accent_color: default_accent_color(),
            language: String::new(),
            download_variant: default_download_variant(),
            auto_start_preset_name: None,
            llama_version: String::new(),
            update_available: None,
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

    /// 返回配置目录路径（下载功能共用）
    pub fn config_dir(&self) -> &Path {
        &self.config_dir
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

    /// 在指定目录及其子目录中 BFS 查找可执行文件（限深 MAX_DEPTH 层）
    /// 优先级：1. 目录本身  2. 浅层优先；同层中名称含 keyword（忽略大小写）的目录优先，
    /// 再按目录名忽略大小写字典序 —— 保证确定性。
    /// 兼容旧行为：keyword 子目录 1 层的匹配仍最先被检查。
    fn find_exe_recursive(&self, dir: &Path, exe_name: &str, keyword: &str) -> Option<PathBuf> {
        const MAX_DEPTH: usize = 4; // 相对给定目录的最大子目录深度

        // 1. 先在目录本身查找
        if let Some(path) = Self::find_exe_in_dir(dir, exe_name) {
            return Some(path);
        }

        // 2. BFS：入队 dir 的子目录（深度 1），逐层弹出检查
        let keyword_lower = keyword.to_lowercase();
        let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::new();
        for d in Self::list_subdirs_sorted(dir, &keyword_lower) {
            queue.push_back((d, 1));
        }

        while let Some((current, depth)) = queue.pop_front() {
            if let Some(path) = Self::find_exe_in_dir(&current, exe_name) {
                return Some(path);
            }
            // 未超过最大深度时入队其子目录
            if depth < MAX_DEPTH {
                for sub in Self::list_subdirs_sorted(&current, &keyword_lower) {
                    queue.push_back((sub, depth + 1));
                }
            }
        }

        None
    }

    /// 列出指定目录下的子目录（read_dir 错误静默跳过）
    /// 排序规则：名称含 keyword（忽略大小写）者优先，再按目录名忽略大小写字典序（确定性）
    fn list_subdirs_sorted(dir: &Path, keyword_lower: &str) -> Vec<PathBuf> {
        let mut subdirs: Vec<PathBuf> = fs::read_dir(dir)
            .ok()
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().is_dir())
                    .map(|e| e.path())
                    .collect()
            })
            .unwrap_or_default();
        subdirs.sort_by(|a, b| {
            let name = |p: &Path| -> String {
                p.file_name()
                    .map(|n| n.to_string_lossy().to_lowercase())
                    .unwrap_or_default()
            };
            let a_name = name(a);
            let b_name = name(b);
            let a_kw = a_name.contains(keyword_lower) as u8;
            let b_kw = b_name.contains(keyword_lower) as u8;
            b_kw.cmp(&a_kw).then_with(|| a_name.cmp(&b_name))
        });
        subdirs
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 拼接平台对应的 exe 文件名
    fn exe_filename(name: &str) -> String {
        if cfg!(target_os = "windows") {
            format!("{}.exe", name)
        } else {
            name.to_string()
        }
    }

    /// 创建目录（含父目录）并写入一个 dummy 可执行文件
    fn make_exe(dir: &Path, name: &str) -> PathBuf {
        fs::create_dir_all(dir).expect("create dir");
        let p = dir.join(name);
        fs::write(&p, b"fake exe").expect("write dummy exe");
        p
    }

    fn manager() -> SettingsManager {
        SettingsManager::new()
    }

    /// 回归：目录本身包含 exe → 直接返回
    #[test]
    fn find_exe_recursive_dir_itself() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let exe = make_exe(tmp.path(), &exe_filename("llama-server"));
        let found = manager().find_exe_recursive(tmp.path(), "llama-server", "llama");
        assert_eq!(found, Some(exe));
    }

    /// 回归：keyword 命名的子目录 1 层深度包含 exe → 返回
    #[test]
    fn find_exe_recursive_keyword_subdir_one_level() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let exe = make_exe(
            &tmp.path().join("llama-b10549"),
            &exe_filename("llama-server"),
        );
        let found = manager().find_exe_recursive(tmp.path(), "llama-server", "llama");
        assert_eq!(found, Some(exe));
    }

    /// 新能力：深路径 llama/llama-b10549/bin/llama-server（深度 3）→ 命中
    #[test]
    fn find_exe_recursive_deep_path_depth_3() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let exe = make_exe(
            &tmp.path().join("llama").join("llama-b10549").join("bin"),
            &exe_filename("llama-server"),
        );
        let found = manager().find_exe_recursive(tmp.path(), "llama-server", "llama");
        assert_eq!(found, Some(exe));
    }

    /// 限深生效：深度 5 的目录中的 exe → 不命中
    #[test]
    fn find_exe_recursive_depth_5_not_found() {
        let tmp = tempfile::tempdir().expect("tempdir");
        // 深度：a(1) / b(2) / c(3) / d(4) / e(5)，exe 位于深度 5 的目录
        let exe = make_exe(
            &tmp.path().join("a").join("b").join("c").join("d").join("e"),
            &exe_filename("llama-server"),
        );
        assert!(exe.exists());
        let found = manager().find_exe_recursive(tmp.path(), "llama-server", "llama");
        assert_eq!(found, None);
    }

    /// download_variant serde 默认值：旧配置（缺字段）反序列化后为 "cpu"
    #[test]
    fn app_settings_download_variant_default() {
        assert_eq!(default_download_variant(), "cpu");
        let json = r#"{"server_path":"","host":"127.0.0.1","port":8080,"parallel_slots":1,"model_path":"","mmproj_path":"","temperature":0.8,"top_p":0.95,"top_k":40,"repeat_penalty":1.1,"presence_penalty":0.0,"kv_offload":false,"cache_type_k":"f16","cache_type_v":"f16","kv_mlock":false,"kv_mmap":true,"kv_unified":false,"gpu_layers_mode":"auto","split_mode":"none","tensor_split":"","cpu_moe":false,"n_cpu_moe":0,"rpc_server_path":"","rpc_host":"127.0.0.1","rpc_port":50052,"rpc_threads":8,"rpc_device":"","rpc_cache":false,"verbose":false}"#;
        let settings: AppSettings = serde_json::from_str(json).expect("旧格式配置应可反序列化");
        assert_eq!(settings.download_variant, "cpu");
    }
}
