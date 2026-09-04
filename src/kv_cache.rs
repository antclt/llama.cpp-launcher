use crate::config::settings::AppSettings;
use crate::i18n;
use std::fs;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::path::Path;

/// GGUF 模型信息摘要
#[derive(Debug)]
pub struct GgufInfo {
    /// 层数 (Block Count / llm.num_hidden_layers)
    pub block_count: usize,
    /// KV头数 (attention.key_head_count)
    pub kv_head_count: usize,
    /// 头维度 (head_dim.k / attention.head_dim)
    pub head_dim: usize,
    /// Embedding 维度 ({arch}.embedding_length)
    pub embedding_length: usize,
    /// 模型文件大小（字节）
    pub file_size: u64,
    /// 全注意力层比例 (full_attention_interval)
    /// 混合架构（如 Qwen3.6/3.8）中只有部分层使用 KV 缓存
    /// None 表示传统架构（所有层都有 KV 缓存）
    pub full_attention_interval: Option<f64>,
}

/// 解析 GGUF 元数据中的 block_count（层数）
fn resolve_block_count(
    arch: &str,
    kv: &std::collections::BTreeMap<String, serde_json::Value>,
    lang: &i18n::Language,
) -> Result<usize, String> {
    let block_key = format!("{}.block_count", arch);
    kv.get(&block_key)
        .and_then(|v| v.as_u64())
        .ok_or_else(|| {
            format!(
                "{} ({})",
                i18n::t(i18n::Key::ErrKvGgufBlocks, lang),
                block_key
            )
        })
        .map(|v| v as usize)
}

/// 从 GGUF 文件中读取模型信息
pub fn read_gguf_info(file_path: &Path, lang: &i18n::Language) -> Result<GgufInfo, String> {
    // 获取文件 size
    let file_size = fs::metadata(file_path)
        .map(|m| m.len())
        .map_err(|e| format!("{}: {}", i18n::t(i18n::Key::ErrKvModelMeta, lang), e))?;

    // 打开 GGUF 容器
    let file_str = file_path
        .to_str()
        .ok_or_else(|| i18n::t(i18n::Key::ErrKvPathInvalid, lang).to_string())?;

    let mut container = gguf_rs::get_gguf_container(file_str)
        .map_err(|e| format!("{}: {}", i18n::t(i18n::Key::ErrKvGgufOpen, lang), e))?;

    // 解码元数据（使用较大 max_array_size 以读取完整 token list）
    let model = container
        .decode()
        .map_err(|e| format!("{}: {}", i18n::t(i18n::Key::ErrKvGgufDecode, lang), e))?;

    let kv = model.metadata();

    // 读取架构名称
    let arch = kv
        .get("general.architecture")
        .and_then(|v| v.as_str())
        .ok_or_else(|| i18n::t(i18n::Key::ErrKvGgufArch, lang).to_string())?;

    // 读取 block_count (层数)
    let block_count = resolve_block_count(arch, kv, lang)?;

    // 读取 KV head count (fallback: attention.head_count_kv → Qwen, attention.head_count)
    let kv_head_key = format!("{}.attention.key_head_count", arch);
    let kv_head_fallback_qwen = format!("{}.attention.head_count_kv", arch);
    let kv_head_fallback = format!("{}.attention.head_count", arch);
    let kv_head_count = kv
        .get(&kv_head_key)
        .or_else(|| kv.get(&kv_head_fallback_qwen))
        .or_else(|| kv.get(&kv_head_fallback))
        .and_then(|v| v.as_u64())
        .ok_or_else(|| {
            format!(
                "{} ({} {} / {} / {})",
                i18n::t(i18n::Key::ErrKvGgufKvHeads, lang),
                i18n::t(i18n::Key::KvTriedKeys, lang),
                kv_head_key,
                kv_head_fallback_qwen,
                kv_head_fallback
            )
        })? as usize;

    // 读取 head dim (fallback: attention.key_length)
    let head_dim_key = format!("{}.head_dim", arch);
    let head_dim_fallback = format!("{}.attention.key_length", arch);
    let head_dim = kv
        .get(&head_dim_key)
        .or_else(|| kv.get(&head_dim_fallback))
        .and_then(|v| v.as_u64())
        .ok_or_else(|| {
            format!(
                "{} ({} {} / {})",
                i18n::t(i18n::Key::ErrKvGgufHeadDim, lang),
                i18n::t(i18n::Key::KvTriedKeys, lang),
                head_dim_key,
                head_dim_fallback
            )
        })? as usize;

    // 读取 embedding length
    let emb_key = format!("{}.embedding_length", arch);
    let embedding_length = kv
        .get(&emb_key)
        .and_then(|v| v.as_u64())
        .ok_or_else(|| format!("{} ({})", i18n::t(i18n::Key::ErrKvGgufEmbed, lang), emb_key))?
        as usize;

    // 读取 full_attention_interval（混合架构特有，如 Qwen3.6/3.8）
    // None 表示传统架构（所有层都有 KV 缓存）
    // Some(value) 表示混合架构，只有 value 比例的层使用 KV 缓存
    let full_attention_interval_key = format!("{}.full_attention_interval", arch);
    let full_attention_interval = kv
        .get(&full_attention_interval_key)
        .and_then(|v| v.as_f64());

    log::info!(
        "[read_gguf_info] full_attention_interval: {:?} (key: {})",
        full_attention_interval,
        full_attention_interval_key
    );

    Ok(GgufInfo {
        block_count,
        kv_head_count,
        head_dim,
        embedding_length,
        file_size,
        full_attention_interval,
    })
}

/// 执行 llama-server --list-devices 并解析空闲显存（MiB）
pub fn get_free_gpu_mib(server_path: &Path, lang: &i18n::Language) -> Result<u64, String> {
    let exe = Path::new(&server_path);
    if !exe.exists() {
        return Err(format!(
            "{}: {:?}",
            i18n::t(i18n::Key::ErrKvServerMissing, lang),
            server_path
        ));
    }

    // 使用 llama-server 所在目录的同名可执行文件来查询设备
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("--list-devices")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x0800_0000u32); // CREATE_NO_WINDOW

    let output = cmd
        .output()
        .map_err(|e| format!("{}: {}", i18n::t(i18n::Key::ErrKvListDevices, lang), e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // 解析 stdout 中的所有 "N MiB free" 模式，并求和
    // 示例输出: (24560 MiB, 24410 MiB free) GPU0 (24560 MiB, 24410 MiB free) GPU1
    if let Some(total) = sum_all_free_mib(&stdout) {
        return Ok(total);
    }

    // 尝试从 stderr 中查找（某些版本可能输出到 stderr）
    if let Some(total) = sum_all_free_mib(&stderr) {
        return Ok(total);
    }

    Err(format!(
        "{}:\n{}",
        i18n::t(i18n::Key::ErrKvParseVram, lang),
        stdout
    ))
}

/// 从一行中提取所有 "N MiB free" 模式对应的数字（MiB）
fn extract_all_free_mib_from_line(line: &str) -> Vec<u64> {
    let mut results = Vec::new();
    let lower = line.to_lowercase();
    let mut search_start = 0;

    // 循环查找所有 "mib free" 出现的位置
    while let Some(pos) = lower[search_start..].find("mib free") {
        let absolute_pos = search_start + pos;
        // 从 "mib" 之前开始向前查找数字
        let prefix = &line[..absolute_pos];
        let trimmed = prefix.trim_end();

        if let Some(last_space) = trimmed.rfind(' ') {
            let num_str = &trimmed[last_space + 1..];
            if let Ok(n) = num_str.parse::<u64>() {
                results.push(n);
            }
        } else if !trimmed.is_empty() {
            // 没有前导空格，尝试解析整行（不太可能）
            if let Ok(n) = trimmed.parse::<u64>() {
                results.push(n);
            }
        }

        // 移动搜索起点，避免重复匹配
        search_start = absolute_pos + "mib free".len();
    }

    results
}

/// 在文本中搜索所有 "N MiB free" 模式，返回所有 N 的总和（MiB）
fn sum_all_free_mib(text: &str) -> Option<u64> {
    let mut total = 0u64;
    let mut found_any = false;

    for line in text.lines() {
        let values = extract_all_free_mib_from_line(line);
        for v in values {
            total = total.saturating_add(v);
            found_any = true;
        }
    }

    if found_any {
        Some(total)
    } else {
        None
    }
}

/// KV 缓存类型 → 精度字节数（f64 以支持量化类型的非整数字节）
fn cache_type_precision_bytes(cache_type: &str) -> f64 {
    match cache_type {
        "f32" => 4.0,
        "f16" | "bf16" => 2.0,
        "q8_0" => 1.0,
        "q5_0" | "q5_1" => 0.625,          // 5 bits per element
        "q4_0" | "q4_1" | "iq4_nl" => 0.5, // 4 bits per element
        "turbo4" => 0.5625,                // 4.5 bits per element (3.8x compression)
        "turbo3" => 0.4375,                // 3.5 bits per element (4.9x compression)
        "turbo2" => 0.3125,                // 2.5 bits per element (6.4x compression)
        _ => 2.0,                          // 默认 f16
    }
}

/// 计算最大可用上下文（k 为单位）
///
/// 公式：
///   Compute Buffer = parallel_slots × block_count × embedding_length × batch_size_actual × coef
///   coef = 2.5 (Flash Attention 启用) / 3.0 (Flash Attention 禁用)
///   单 token KV 占用 = kv_head_count × head_dim × (precision_k + precision_v) × kv_cache_layers
///   kv_cache_layers = block_count (传统架构) / block_count × full_attention_interval (混合架构)
///   最大 token 数 = ((GPU 空闲显存 - 模型文件) - Compute Buffer) / 单 token KV 占用 × kv_cache_ratio
///   返回值 = 最大 token 数 / 1024 （单位 k）
pub fn calc_max_context(gguf: &GgufInfo, settings: &AppSettings, free_mib: u64) -> u64 {
    // Flash Attention 启用时降低 Compute Buffer 系数（f16=2B × 1.25x）
    // Flash Attention 禁用时保持原始系数（f16=2B × 1.5x）
    let compute_buffer_coef: f64 = match settings.flash_attn.as_str() {
        "on" | "auto" => 2.5, // Flash Attention 启用：f16(2B) × 1.25x
        _ => 3.0,             // Flash Attention 禁用或空：f16(2B) × 1.5x
    };

    // Compute Buffer（字节）= parallel_slots * block_count * embedding_length * batch_size_actual × coef
    let compute_buffer_bytes = ((settings.parallel_slots as f64)
        * (gguf.block_count as f64)
        * (gguf.embedding_length as f64)
        * (settings.batch_size_actual() as f64)
        * compute_buffer_coef) as u64;

    // 模型文件占用（MiB）
    let model_mib = gguf.file_size / (1024 * 1024);

    // GPU 空闲显存扣除模型文件后的可用空间（字节）
    let usable_bytes = free_mib
        .saturating_sub(model_mib)
        .saturating_mul(1024)
        .saturating_mul(1024);

    // 扣掉 Compute Buffer 后剩余给 KV 缓存的空间（字节）
    let kv_bytes = usable_bytes.saturating_sub(compute_buffer_bytes);

    if kv_bytes == 0 {
        return 0;
    }

    // K 和 V 的精度字节数
    let precision_k = cache_type_precision_bytes(&settings.cache_type_k);
    let precision_v = cache_type_precision_bytes(&settings.cache_type_v);

    // 计算实际需要 KV 缓存的层数
    // 混合架构（如 Qwen3.5/3.6/3.8）只有部分层使用 KV 缓存
    // full_attention_interval 是间隔值（如 4 表示每 4 层中有 1 层需要 KV 缓存）
    let kv_cache_layers = match gguf.full_attention_interval {
        Some(interval) => {
            // 混合架构：每 interval 层中有 1 层需要 KV 缓存
            let layers = (gguf.block_count as f64 / interval).ceil() as usize;
            log::info!("[calc_max_context] 混合架构: block_count={}, full_attention_interval={}, kv_cache_layers={}",
                gguf.block_count, interval, layers);
            layers
        }
        None => {
            // 传统架构：所有层都有 KV 缓存
            gguf.block_count
        }
    };

    // 单个 token 的 KV 缓存占用（字节）= kv_head_count × head_dim × (precision_k + precision_v) × kv_cache_layers
    let per_token_kv_bytes = gguf.kv_head_count as f64
        * gguf.head_dim as f64
        * (precision_k + precision_v)
        * kv_cache_layers as f64;

    if per_token_kv_bytes <= 0.0 {
        return 0;
    }

    // 最大 token 数 = 剩余字节 / 单 token 占用 × kv_cache_ratio
    let max_tokens = (kv_bytes as f64) / per_token_kv_bytes * settings.kv_cache_ratio as f64;

    // 返回 k 单位（1k = 1024）
    (max_tokens / 1024.0) as u64
}

/// 计算 KV 缓存可用空间
/// 公式: (GPU空闲显存 - 模型文件大小) - (并发数量 × block_count × embedding_length × 物理批次大小 × coef)
/// coef = 2.5 (Flash Attention 启用) / 3.0 (Flash Attention 禁用)
pub fn calc_kv_cache_space(
    gguf: &GgufInfo,
    settings: &AppSettings,
    free_mib: u64,
    lang: &i18n::Language,
) -> String {
    // Flash Attention 启用时降低 Compute Buffer 系数（f16=2B × 1.25x）
    // Flash Attention 禁用时保持原始系数（f16=2B × 1.5x）
    let compute_buffer_coef: f64 = match settings.flash_attn.as_str() {
        "on" | "auto" => 2.5, // Flash Attention 启用：f16(2B) × 1.25x
        _ => 3.0,             // Flash Attention 禁用或空：f16(2B) × 1.5x
    };

    // Compute Buffer（字节）= parallel_slots * block_count * embedding_length * batch_size_actual × coef
    let compute_buffer_bytes = ((settings.parallel_slots as f64)
        * (gguf.block_count as f64)
        * (gguf.embedding_length as f64)
        * (settings.batch_size_actual() as f64)
        * compute_buffer_coef) as u64;

    // 模型文件占用（MiB）
    let model_mib = gguf.file_size / (1024 * 1024);

    // GPU空闲显存扣除模型文件后的可用空间（MiB）
    let usable_mib = free_mib.saturating_sub(model_mib);

    // Compute Buffer（MiB）
    let compute_buffer_mib = compute_buffer_bytes / (1024 * 1024);

    if compute_buffer_mib > usable_mib {
        let over = compute_buffer_mib - usable_mib;
        let over_gb = over as f64 / 1024.0;
        format!(
            "{} {} MB ({:.1} GB)",
            i18n::t(i18n::Key::KvCacheOverBudget, lang),
            over,
            over_gb
        )
    } else {
        let remaining = usable_mib - compute_buffer_mib;
        let remaining_gb = remaining as f64 / 1024.0;
        format!("{} MB ({:.1} GB)", remaining, remaining_gb)
    }
}

/// Facade function：聚合读取 GGUF + GPU 信息 → 计算并格式化结果
pub fn calc_and_format(settings: &AppSettings, lang: &i18n::Language) -> Result<String, String> {
    log::info!("[calc_and_format] 开始计算 KV 缓存空间");
    log::info!("[calc_and_format] model_path = {:?}", settings.model_path);
    log::info!("[calc_and_format] server_path = {:?}", settings.server_path);

    // 1. 读取 GGUF 模型信息
    let gguf = read_gguf_info(&settings.model_path, lang)?;
    log::info!("[calc_and_format] GGUF info: block_count={}, kv_head_count={}, head_dim={}, embedding_length={}, file_size={} bytes, full_attention_interval={:?}",
        gguf.block_count, gguf.kv_head_count, gguf.head_dim, gguf.embedding_length, gguf.file_size, gguf.full_attention_interval);

    // 2. 获取空闲显存
    let free_mib = get_free_gpu_mib(&settings.server_path, lang)?;
    log::info!("[calc_and_format] GPU 空闲显存: {} MiB", free_mib);

    // 3. 计算并格式化
    let result = calc_kv_cache_space(&gguf, settings, free_mib, lang);
    log::info!(
        "[calc_and_format] flash_attn={}, KV 缓存计算结果: {}",
        settings.flash_attn,
        result
    );
    Ok(result)
}

/// Facade function：聚合读取 GGUF + GPU 信息 → 返回最大上下文（k 单位）
pub fn calc_max_context_facade(
    settings: &AppSettings,
    lang: &i18n::Language,
) -> Result<usize, String> {
    log::info!("[calc_max_context_facade] 开始计算最大上下文");

    // 1. 读取 GGUF 模型信息
    let gguf = read_gguf_info(&settings.model_path, lang)?;
    log::info!("[calc_max_context_facade] GGUF info: block_count={}, kv_head_count={}, head_dim={}, embedding_length={}, file_size={} bytes, full_attention_interval={:?}",
        gguf.block_count, gguf.kv_head_count, gguf.head_dim, gguf.embedding_length, gguf.file_size, gguf.full_attention_interval);

    // 2. 获取空闲显存
    let free_mib = get_free_gpu_mib(&settings.server_path, lang)?;
    log::info!("[calc_max_context_facade] GPU 空闲显存: {} MiB", free_mib);

    // 3. 计算最大上下文（k 单位）
    let max_ctx_k = calc_max_context(&gguf, settings, free_mib);
    log::info!(
        "[calc_max_context_facade] flash_attn={}, cache_type_k={}, cache_type_v={}, kv_cache_ratio={}",
        settings.flash_attn,
        settings.cache_type_k,
        settings.cache_type_v,
        settings.kv_cache_ratio
    );
    log::info!("[calc_max_context_facade] 最大可用上下文: {}k", max_ctx_k);
    Ok(max_ctx_k as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_single_gpu() {
        let line = "  ROCm0: Radeon RX 7900 XTX (24560 MiB, 24524 MiB free)";
        let values = extract_all_free_mib_from_line(line);
        assert_eq!(values, vec![24524]);
    }

    #[test]
    fn test_extract_multiple_gpus_same_line() {
        let line = "(24560 MiB, 24410 MiB free) GPU0 (24560 MiB, 24410 MiB free) GPU1";
        let values = extract_all_free_mib_from_line(line);
        assert_eq!(values, vec![24410, 24410]);
    }

    #[test]
    fn test_sum_all_free_mib() {
        let text = "Available devices:\n  ROCm0: Radeon RX 7900 XTX (24560 MiB, 24524 MiB free)\n  ROCm1: Radeon RX 7900 XTX (24560 MiB, 24524 MiB free)";
        let total = sum_all_free_mib(text);
        assert_eq!(total, Some(49048));
    }

    #[test]
    fn test_sum_all_free_mib_no_match() {
        let text = "No GPU devices found";
        let total = sum_all_free_mib(text);
        assert_eq!(total, None);
    }

    #[test]
    fn test_resolve_block_count_from_metadata() {
        // 普通架构从 metadata 读取 block_count
        let mut kv = std::collections::BTreeMap::new();
        kv.insert("llama.block_count".to_string(), serde_json::json!(42));
        let result = resolve_block_count("llama", &kv, &i18n::Language::Zh);
        assert_eq!(result, Ok(42));
    }

    #[test]
    fn test_resolve_block_count_missing_key() {
        // 普通架构缺少 block_count 字段时报错，错误消息含键名
        let kv = std::collections::BTreeMap::new();
        let block_key = "llama.block_count".to_string();
        let err = resolve_block_count("llama", &kv, &i18n::Language::Zh).unwrap_err();
        assert!(err.contains("块数"));
        assert!(err.contains(&block_key));
    }
}
