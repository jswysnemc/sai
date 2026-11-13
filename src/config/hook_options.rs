//! 控制型钩子的可选配置，与旧 HookItem 平铺保存。
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct HookOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matcher: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub r#async: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_windows: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_tool: Option<String>,
    #[serde(default)]
    pub mcp_arguments: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_message: Option<String>,
    #[serde(skip)]
    pub runtime: Option<crate::hooks::sources::HookSource>,
}
