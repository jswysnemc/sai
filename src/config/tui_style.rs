use clap::ValueEnum;
use serde::{Deserialize, Serialize};

/// 终端界面的展示样式；不改变全屏偏好或会话执行行为。
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum TuiStyle {
    #[default]
    Classic,
    Deepseek,
}

impl TuiStyle {
    /// 【终端】【样式解析】解析命令参数，不接受未知样式。
    /// 参数: `value` 为样式名称；返回: 已识别样式或空值。
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "classic" => Some(Self::Classic),
            "deepseek" => Some(Self::Deepseek),
            _ => None,
        }
    }

    /// 【终端】【样式名称】返回配置文件和命令使用的稳定名称。
    /// 参数: 当前样式；返回: 样式名称。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Classic => "classic",
            Self::Deepseek => "deepseek",
        }
    }

    /// 【终端】【样式切换】返回另一种展示样式。
    /// 参数: 当前样式；返回: 切换后的样式。
    pub fn toggled(self) -> Self {
        match self {
            Self::Classic => Self::Deepseek,
            Self::Deepseek => Self::Classic,
        }
    }
}

#[cfg(test)]
#[path = "tui_style_tests.rs"]
mod tests;
