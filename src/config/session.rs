use serde::{Deserialize, Serialize};

/// 会话标题与自动命名相关配置。
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct SessionConfig {
    /// 是否在新建会话的首轮自动生成标题。
    #[serde(default = "default_true")]
    pub auto_title_enabled: bool,
    /// 自动标题专用供应商；空则使用当前会话供应商。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub auto_title_provider_id: String,
    /// 自动标题专用模型；空则使用当前会话模型。
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub auto_title_model: String,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            auto_title_enabled: true,
            auto_title_provider_id: String::new(),
            auto_title_model: String::new(),
        }
    }
}

fn default_true() -> bool {
    true
}
