use super::defaults::*;
use serde::{Deserialize, Deserializer, Serialize};

/// TUI 折叠预览：保留开头、保留首尾，或全部省略。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum FoldPreviewMode {
    /// 只保留开头若干行
    Head,
    /// 保留开头与结尾
    #[default]
    Ends,
    /// 折叠时不保留正文
    Hidden,
}

impl FoldPreviewMode {
    /// 【显示配置】【折叠模式】把配置文本解析为折叠预览模式。
    ///
    /// 参数:
    /// - `value`: 配置文本
    ///
    /// 返回:
    /// - 已知取值；未知文本回退为首尾模式
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "head" | "开头" | "保留开头" => Self::Head,
            "hidden" | "omit" | "全部省略" | "省略" => Self::Hidden,
            "ends" | "首尾" | "保留首尾" => Self::Ends,
            _ => Self::Ends,
        }
    }

    /// 【显示配置】【折叠标识】返回写入配置文件的稳定取值。
    ///
    /// 返回:
    /// - `head` / `ends` / `hidden`
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Head => "head",
            Self::Ends => "ends",
            Self::Hidden => "hidden",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DisplayConfig {
    /// TUI 启动时使用全局渲染，Ctrl+O 可临时切换
    pub fullscreen: bool,
    /// 将终端公式渲染成图片
    pub math_images: bool,
    /// 将终端 Mermaid 代码块渲染成图片
    pub mermaid_images: bool,
    #[serde(default = "default_reasoning_display")]
    pub reasoning: String,
    #[serde(default = "default_tool_call_display")]
    pub tool_calls: String,
    #[serde(default = "default_true")]
    pub readable_tool_names: bool,
    #[serde(default = "default_true")]
    pub wait_show_model: bool,
    #[serde(default = "default_true")]
    pub wait_show_thinking_level: bool,
    #[serde(default = "default_repl_transcript_row_cap")]
    pub repl_transcript_row_cap: usize,
    /// 折叠预览保留策略
    #[serde(default)]
    pub fold_preview: FoldPreviewMode,
    /// 折叠时保留的开头行数
    #[serde(default = "default_fold_head_lines")]
    pub fold_head_lines: usize,
    /// 折叠时保留的结尾行数；仅首尾模式使用
    #[serde(default = "default_fold_tail_lines")]
    pub fold_tail_lines: usize,
}

#[derive(Debug, Clone, Deserialize)]
struct RawDisplayConfig {
    #[serde(default)]
    fullscreen: Option<bool>,
    #[serde(default)]
    math_images: Option<bool>,
    #[serde(default)]
    mermaid_images: Option<bool>,
    #[serde(default)]
    reasoning: Option<String>,
    #[serde(default)]
    tool_calls: Option<String>,
    #[serde(default)]
    show_reasoning: Option<bool>,
    #[serde(default)]
    reasoning_mode: Option<String>,
    #[serde(default)]
    show_tool_details: Option<bool>,
    #[serde(default)]
    readable_tool_names: Option<bool>,
    #[serde(default)]
    wait_show_model: Option<bool>,
    #[serde(default)]
    wait_show_thinking_level: Option<bool>,
    #[serde(default)]
    repl_transcript_row_cap: Option<usize>,
    #[serde(default)]
    fold_preview: Option<String>,
    #[serde(default)]
    fold_head_lines: Option<usize>,
    #[serde(default)]
    fold_tail_lines: Option<usize>,
}

impl DisplayConfig {
    /// 【显示配置】【行数夹紧】把折叠保留行数限制在可读范围内。
    ///
    /// 参数:
    /// - `head`: 开头行数
    ///
    /// 返回:
    /// - 1..=24
    pub fn clamp_head(head: usize) -> usize {
        head.clamp(1, 24)
    }

    /// 【显示配置】【行数夹紧】把折叠尾部行数限制在可读范围内。
    ///
    /// 参数:
    /// - `tail`: 结尾行数
    ///
    /// 返回:
    /// - 0..=24
    pub fn clamp_tail(tail: usize) -> usize {
        tail.min(24)
    }
}

impl<'de> Deserialize<'de> for DisplayConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawDisplayConfig::deserialize(deserializer)?;
        let reasoning = raw.reasoning.unwrap_or_else(|| {
            if raw.show_reasoning == Some(false) {
                "hidden".to_string()
            } else {
                raw.reasoning_mode.unwrap_or_else(default_reasoning_display)
            }
        });
        let tool_calls = raw.tool_calls.unwrap_or_else(|| {
            if raw.show_tool_details == Some(true) {
                "full".to_string()
            } else {
                default_tool_call_display()
            }
        });
        Ok(Self {
            fullscreen: raw.fullscreen.unwrap_or(true),
            math_images: raw.math_images.unwrap_or(true),
            mermaid_images: raw.mermaid_images.unwrap_or(true),
            reasoning,
            tool_calls,
            readable_tool_names: raw.readable_tool_names.unwrap_or_else(default_true),
            wait_show_model: raw.wait_show_model.unwrap_or_else(default_true),
            wait_show_thinking_level: raw.wait_show_thinking_level.unwrap_or_else(default_true),
            repl_transcript_row_cap: raw
                .repl_transcript_row_cap
                .unwrap_or_else(default_repl_transcript_row_cap),
            fold_preview: raw
                .fold_preview
                .as_deref()
                .map(FoldPreviewMode::parse)
                .unwrap_or_default(),
            fold_head_lines: DisplayConfig::clamp_head(
                raw.fold_head_lines.unwrap_or_else(default_fold_head_lines),
            ),
            fold_tail_lines: DisplayConfig::clamp_tail(
                raw.fold_tail_lines.unwrap_or_else(default_fold_tail_lines),
            ),
        })
    }
}
