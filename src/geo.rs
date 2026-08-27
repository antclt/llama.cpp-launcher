//! 地理位置检测模块
//!
//! 通过系统语言和时区检测是否在中国大陆，用于决定是否优先使用镜像源。
//! 零外部依赖，仅读取系统环境变量或执行系统命令。

/// 检测当前系统是否在中国大陆
///
/// 检测策略（按优先级）：
/// 1. Windows: 检测时区是否为 Asia/Shanghai 或 Asia/Chongqing
/// 2. Linux/macOS: 检测 LANG 环境变量是否以 "zh_CN" 开头
/// 3. 通用: 检测 LC_ALL / LC_MESSAGES 环境变量
pub fn is_china_mainland() -> bool {
    // 策略 1: Windows 时区检测
    #[cfg(target_os = "windows")]
    {
        if is_windows_china_timezone() {
            return true;
        }
    }

    // 策略 2: Unix 语言环境检测
    #[cfg(not(target_os = "windows"))]
    {
        if is_unix_china_locale() {
            return true;
        }
    }

    false
}

/// Windows: 检测时区是否为中国大陆
///
/// 使用 `tzutil /g` 命令获取当前时区 ID
#[cfg(target_os = "windows")]
fn is_windows_china_timezone() -> bool {
    use std::process::Command;

    // 执行 tzutil /g 获取时区 ID（如 "China Standard Time"）
    let output = Command::new("tzutil").arg("/g").output();

    if let Ok(output) = output {
        if output.status.success() {
            let tz = String::from_utf8_lossy(&output.stdout)
                .trim()
                .to_lowercase();

            // 检查是否为中国时区
            // 常见的中国时区 ID: "china standard time", "qinghai standard time" (青海)
            if tz.contains("china") || tz.contains("qinghai") {
                return true;
            }
        }
    }

    false
}

/// Linux/macOS: 检测语言环境是否为中国大陆
#[cfg(not(target_os = "windows"))]
fn is_unix_china_locale() -> bool {
    // 检查 LANG 环境变量 (最常用)
    if let Ok(lang) = std::env::var("LANG") {
        if lang.starts_with("zh_CN") {
            return true;
        }
    }

    // 检查 LC_ALL 环境变量 (优先级最高)
    if let Ok(lc_all) = std::env::var("LC_ALL") {
        if lc_all.starts_with("zh_CN") {
            return true;
        }
    }

    // 检查 LC_MESSAGES 环境变量
    if let Ok(lc_messages) = std::env::var("LC_MESSAGES") {
        if lc_messages.starts_with("zh_CN") {
            return true;
        }
    }

    false
}

/// 判断是否应该优先使用镜像源
///
/// 返回 `true` 表示应该先尝试镜像，再尝试官方源；
/// 返回 `false` 表示先尝试官方源，失败后再回退到镜像。
pub fn should_use_mirror_first() -> bool {
    is_china_mainland()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_china_mainland() {
        // 这个测试在不同环境下结果不同
        // 主要验证函数能正常运行
        let _result = is_china_mainland();
    }

    #[test]
    fn test_should_use_mirror_first() {
        // 验证函数能正常运行
        let _result = should_use_mirror_first();
    }
}
