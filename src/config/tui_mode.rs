use clap::ValueEnum;
use serde::{Deserialize, Serialize};

/// 终端启动布局；全屏仅使用统一执行链，内联保留终端滚动输出。
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum TuiMode {
    #[serde(alias = "classic")]
    #[value(alias = "classic")]
    Inline,
    #[default]
    #[serde(alias = "deepseek")]
    #[value(alias = "deepseek")]
    Fullscreen,
}

impl TuiMode {
    /// 【终端】【样式解析】解析命令参数，不接受未知样式。
    /// 参数: `value` 为样式名称；返回: 已识别样式或空值。
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "inline" => Some(Self::Inline),
            "fullscreen" => Some(Self::Fullscreen),
            _ => None,
        }
    }

    /// 【终端】【样式名称】返回配置文件和命令使用的稳定名称。
    /// 参数: 当前样式；返回: 样式名称。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Inline => "inline",
            Self::Fullscreen => "fullscreen",
        }
    }

    /// 【终端】【样式切换】返回另一种展示样式。
    /// 参数: 当前样式；返回: 切换后的样式。
    pub fn toggled(self) -> Self {
        match self {
            Self::Inline => Self::Fullscreen,
            Self::Fullscreen => Self::Inline,
        }
    }
}

#[cfg(test)]
#[path = "tui_mode_tests.rs"]
mod tests;
