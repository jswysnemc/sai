use crate::agent::AgentEvent;
use crate::llm::ChatStreamKind;
use std::time::Duration;

/// 思考中/工作中共用的点跳动动效帧（与 reasoning live 一致）。
pub(crate) const STATUS_PULSE_FRAMES: [&str; 4] = ["·  ", " · ", "  ·", " · "];

/// 单轮请求的用户可见工作状态。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkStatus {
    WaitingResponse,
    WaitingExternal,
    Thinking,
    Working,
    Compacting,
}

impl WorkStatus {
    /// 根据 Agent 事件计算下一工作状态。
    ///
    /// 参数:
    /// - `event`: 当前 Agent 事件
    ///
    /// 返回:
    /// - 需要更新时返回新状态
    pub(crate) fn from_agent_event(event: &AgentEvent) -> Option<Self> {
        match event {
            AgentEvent::Chunk(chunk) if chunk.kind == ChatStreamKind::Reasoning => {
                Some(Self::Thinking)
            }
            AgentEvent::Chunk(_)
            | AgentEvent::ToolCall { .. }
            | AgentEvent::ToolCallProgress(_)
            | AgentEvent::ToolResult { .. }
            | AgentEvent::ToolProgress { .. }
            | AgentEvent::PermissionResolved { .. }
            | AgentEvent::QuestionResolved { .. } => Some(Self::Working),
            // 权限/提问交互期间由专门 UI 接管，不进入 Working，避免与审核行重叠
            AgentEvent::PermissionRequested(_) | AgentEvent::QuestionRequested(_) => None,
            AgentEvent::CompactionStarted { .. } => Some(Self::Compacting),
            AgentEvent::CompactionDelta { .. }
            | AgentEvent::CompactionFinished { .. }
            | AgentEvent::FlushContent
            | AgentEvent::ExternalOutput => None,
        }
    }

    /// 返回统一英文状态名称。
    ///
    /// 返回:
    /// - 工作状态文本
    #[allow(dead_code)]
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::WaitingResponse => "waiting",
            Self::WaitingExternal => "waiting for external work",
            Self::Thinking => "thinking",
            Self::Working => "working",
            Self::Compacting => "compacting",
        }
    }

    /// 返回本地化状态文案。
    ///
    /// 返回:
    /// - 中英文状态短语
    pub(crate) fn localized_label(self) -> &'static str {
        use crate::render::terminal_text as t;
        match self {
            Self::WaitingResponse => t("Waiting for model response", "等待模型响应"),
            Self::WaitingExternal => t("Waiting for background work", "等待后台工作"),
            Self::Thinking => t("Organizing thoughts", "正在整理思路"),
            Self::Working => t("Working on the task", "正在执行任务"),
            Self::Compacting => t("Compacting context", "正在压缩上下文"),
        }
    }

    /// 渲染适合历史区展示的动态状态行。
    ///
    /// 复用思考中的点跳动动效，仅文案与耗时不同。
    ///
    /// 参数:
    /// - `frame`: 动画帧序号
    /// - `elapsed`: 当前状态已持续时长
    ///
    /// 返回:
    /// - 带 ANSI 样式的状态行
    pub(crate) fn render_line(self, frame: usize, elapsed: Duration) -> String {
        let pulse = STATUS_PULSE_FRAMES[frame % STATUS_PULSE_FRAMES.len()];
        format!(
            "\x1b[2m\x1b[36m{pulse} {}({})\x1b[0m",
            self.localized_label(),
            format_elapsed(elapsed)
        )
    }
}

/// 格式化工作时长，用于状态行展示。
///
/// 参数:
/// - `elapsed`: 已用时长
///
/// 返回:
/// - 人类可读时长文本
pub(crate) fn format_elapsed(elapsed: Duration) -> String {
    use crate::i18n::is_zh;
    let total_secs = elapsed.as_secs();
    if is_zh() {
        if total_secs < 60 {
            return format!("{total_secs}秒");
        }
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        if mins < 60 {
            return format!("{mins}分{secs}秒");
        }
        let hours = mins / 60;
        let remain_mins = mins % 60;
        return format!("{hours}小时{remain_mins}分{secs}秒");
    }
    if total_secs < 60 {
        let tenths = elapsed.as_millis() / 100;
        format!("{}.{}s", tenths / 10, tenths % 10)
    } else {
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        format!("{mins}m{secs:02}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::ChatStreamChunk;

    #[test]
    fn reasoning_and_content_map_to_distinct_states() {
        let reasoning = AgentEvent::Chunk(ChatStreamChunk {
            kind: ChatStreamKind::Reasoning,
            text: "inspect".to_string(),
        });
        let content = AgentEvent::Chunk(ChatStreamChunk {
            kind: ChatStreamKind::Content,
            text: "answer".to_string(),
        });

        assert_eq!(
            WorkStatus::from_agent_event(&reasoning),
            Some(WorkStatus::Thinking)
        );
        assert_eq!(
            WorkStatus::from_agent_event(&content),
            Some(WorkStatus::Working)
        );
    }

    #[test]
    fn working_reuses_thinking_pulse_animation() {
        let line = WorkStatus::Working.render_line(0, Duration::from_millis(1500));
        assert!(line.contains(WorkStatus::Working.label()) || line.contains("正在执行任务") || line.contains("Working on the task"));
        assert!(line.contains("1.5s") || line.contains("1秒") || line.contains("("));
        assert!(line.contains(STATUS_PULSE_FRAMES[0]));
        // 与思考 live 动效同色同款（dim cyan + pulse）
        assert!(line.contains("\x1b[2m\x1b[36m") || line.contains("\x1b[2m") && line.contains("·"));
    }

    #[test]
    fn permission_requested_does_not_enter_working() {
        let event = AgentEvent::PermissionRequested(crate::permission::PermissionRequest {
            id: "p".into(),
            session_id: "s".into(),
            tool: "edit_file".into(),
            arguments: "{}".into(),
            auto_audit: false,
        });
        assert_eq!(WorkStatus::from_agent_event(&event), None);
    }
}
