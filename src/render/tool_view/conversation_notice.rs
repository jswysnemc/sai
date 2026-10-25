use super::ToolView;
use crate::render::{
    status_style::ToolHealth, tool_event_line::tool_status_line, ToolCallDisplayMode,
};

/// 【终端】【会话通知】区分目标续轮与当前轮次内的排队消息交付。
#[derive(Clone, Copy, Debug)]
pub(crate) enum ConversationNoticeKind {
    GoalContinuation,
    QueuedUser,
}

impl ConversationNoticeKind {
    /// 【终端】【通知类型】返回通知工具使用的稳定类型，不依赖显示语言。
    /// 参数: 无；返回: 类型文本。
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::GoalContinuation => "goal_continuation",
            Self::QueuedUser => "queued_user",
        }
    }
}

/// 【终端】【会话通知】使用普通工具样式展示后续消息，图片只在详情展开时预览。
/// 参数: `view` 为通知工具，`mode` 为展示方式；返回: 通知正文，其他工具为空。
pub(super) fn render(view: &ToolView, mode: ToolCallDisplayMode) -> Option<String> {
    if view.name != "conversation_notice" {
        return None;
    }
    if mode == ToolCallDisplayMode::Hidden {
        return Some(String::new());
    }
    // 1. 【终端】【会话通知】只读取内部元数据，消息正文不进行 JSON 解析或翻译
    let arguments = serde_json::from_str::<serde_json::Value>(&view.arguments).unwrap_or_default();
    let goal = arguments["kind"].as_str() == Some("goal_continuation");
    let label = if goal {
        "Continuing goal"
    } else {
        "Queued message delivered"
    };
    let text = view
        .outcome
        .as_ref()
        .map(|outcome| outcome.output.as_str())
        .unwrap_or_default();
    let text = if goal { goal_text(text) } else { text };
    if mode == ToolCallDisplayMode::Summary {
        let subject = text.lines().next().unwrap_or_default().trim();
        let title = if subject.is_empty() {
            label.into()
        } else {
            format!("{label} · {subject}")
        };
        return Some(tool_status_line(&title, "", ToolHealth::Neutral));
    }
    let mut output = tool_status_line(label, "", ToolHealth::Neutral);
    let prefix = if goal { "Goal: " } else { "Message: " };
    let padding = " ".repeat(4 + unicode_width::UnicodeWidthStr::width(prefix));
    for (index, line) in text.split('\n').enumerate() {
        output.push_str(&if index == 0 {
            format!("\n    {prefix}{line}")
        } else {
            format!("\n{padding}{line}")
        });
    }
    // 2. 【终端】【会话通知】沿用有界左对齐预览，不能把图片来源作为原始元数据显示
    if let Some(sources) = arguments["image_sources"]
        .as_array()
        .filter(|sources| !sources.is_empty())
    {
        if crate::render::render_expand::resolve_expanded(false) {
            for source in sources.iter().filter_map(serde_json::Value::as_str) {
                let preview = crate::render::image_preview::render(
                    source,
                    crate::render::fold_text::terminal_wrap_width()
                        .saturating_sub(4)
                        .max(1),
                );
                for line in preview.split('\n') {
                    output.push_str(&format!("\n    {line}"));
                }
            }
        } else {
            output.push_str(&format!(
                "\n    {}",
                crate::render::omitted_line::render_fold_hint(
                    "Image preview",
                    Some(crate::render::omitted_line::FOLD_SHORTCUT)
                )
            ));
        }
    }
    Some(output)
}

/// 【终端】【目标正文】移除内部中英文提示前缀，保留用户目标，不展示模型续轮指令。
/// 参数: `text` 为已面向用户的续轮消息；返回: 目标正文或简洁英文说明。
fn goal_text(text: &str) -> &str {
    let text = text.trim();
    if text.is_empty() || matches!(text, "Goal continuation" | "自动继续当前任务") {
        return "Continuing the current goal";
    }
    [
        "Goal 自动续轮：",
        "Goal 自动续轮:",
        "Goal continuation:",
        "Goal:",
    ]
    .iter()
    .find_map(|prefix| text.strip_prefix(prefix).map(str::trim))
    .unwrap_or(text)
}
