pub mod rpc;
pub mod server;

use crate::i18n;

/// 状态机错误信息：存储 i18n 键（而非已解析文本），渲染时再按 lang 实时解析。
///
/// 设计动机：engine 层的 start()/poll() 不持有语言信息，若在此处直接
/// `i18n::t(key, 固定语言)` 会把错误文案的语言写死。改为存储键、在
/// `status_text(lang)` 渲染边界解析，用户切换语言后错误提示可实时更新。
#[derive(Debug, Clone, PartialEq)]
pub enum ErrorInfo {
    /// 纯 i18n 键错误（无动态详情）
    Key(i18n::Key),
    /// i18n 键 + 动态详情（如系统错误信息 / 进程退出码）
    WithDetail(i18n::Key, String),
}

impl ErrorInfo {
    /// 按指定语言解析为本地化文本
    pub fn text(&self, lang: &i18n::Language) -> String {
        match self {
            ErrorInfo::Key(key) => i18n::t(*key, lang).to_string(),
            ErrorInfo::WithDetail(key, detail) => {
                format!("{}: {}", i18n::t(*key, lang), detail)
            }
        }
    }
}
