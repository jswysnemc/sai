//! 终端输入框样式，旧配置保持原有面板样式。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputStyle {
    #[default]
    Panel,
    Separator,
}

impl InputStyle {
    /// 【输入样式】【配置解析】参数：value 为样式名；返回：已知样式，非法值返回空
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "panel" => Some(Self::Panel),
            "separator" => Some(Self::Separator),
            _ => None,
        }
    }

    /// 【输入样式】【配置名称】参数：当前样式；返回：稳定配置名称
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Panel => "panel",
            Self::Separator => "separator",
        }
    }
}
