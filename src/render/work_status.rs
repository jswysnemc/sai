use crate::agent::AgentEvent;
use crate::llm::ChatStreamKind;
use crate::render::activity_animation::render_activity_line;
use std::time::Duration;

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
            | AgentEvent::ToolCallIdentified { .. }
            | AgentEvent::ToolCallProgress(_)
            | AgentEvent::ToolResult { .. }
            | AgentEvent::ToolResultIdentified { .. }
            | AgentEvent::ToolProgress { .. }
            | AgentEvent::ToolProgressIdentified { .. }
            | AgentEvent::PermissionResolved { .. }
            | AgentEvent::QuestionResolved { .. } => Some(Self::Working),
            // 权限/提问交互期间由专门 UI 接管，不进入 Working，避免与审核行重叠
            AgentEvent::PermissionRequested(_) | AgentEvent::QuestionRequested(_) => None,
            AgentEvent::CompactionStarted { .. } => Some(Self::Compacting),
            AgentEvent::CompactionDelta { .. }
            | AgentEvent::CompactionFinished { .. }
            | AgentEvent::EngineReady { .. }
            | AgentEvent::FlushContent
            | AgentEvent::ExternalOutput => None,
        }
    }

    /// 【终端】【工作状态】返回统一英文状态名称。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 工作状态文本
    #[allow(dead_code)]
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::WaitingResponse => "Waiting",
            Self::WaitingExternal => "Waiting for external work",
            Self::Thinking => "Thinking",
            Self::Working => "Working",
            Self::Compacting => "Compacting",
        }
    }

    /// 【终端】【工作状态】返回动效状态文案。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 英文状态短语
    pub(crate) fn localized_label(self) -> &'static str {
        match self {
            Self::WaitingResponse => "Waiting",
            Self::WaitingExternal => "Waiting",
            Self::Thinking => "Thinking",
            Self::Working => "Working",
            Self::Compacting => "Compacting",
        }
    }

    /// 【终端】【工作状态】渲染适合历史区展示的动态状态行。
    ///
    /// 行首引导点与助手正文共用同一符号，因此状态行与正文落在同一条视觉基线上；
    /// 状态文字使用从左向右的白色余弦流光，并展示本轮整数秒时长。
    ///
    /// 参数:
    /// - `frame`: 动画帧序号
    /// - `elapsed`: 本轮自首次回应起的已持续时长
    ///
    /// 返回:
    /// - 带 ANSI 样式的状态行
    pub(crate) fn render_line(self, frame: usize, elapsed: Duration) -> String {
        render_activity_line(self.localized_label(), &format_elapsed(elapsed), frame)
    }
}

/// 【终端】【工作状态】格式化工作时长。
///
/// 参数:
/// - `elapsed`: 已用时长
///
/// 返回:
/// - 如 `12s` / `1m05s`
pub(crate) fn format_elapsed(elapsed: Duration) -> String {
    let total_secs = elapsed.as_secs();
    if total_secs < 60 {
        format!("{total_secs}s")
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
    use crate::render::activity_animation::strip_ansi_for_test;

    /// 【终端】【工作状态测试】验证推理与正文事件映射到不同状态。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
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

    /// 【终端】【工作状态测试】验证 Working 使用白色流光和整数秒。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn working_uses_white_shimmer_and_integer_seconds() {
        let line = WorkStatus::Working.render_line(0, Duration::from_millis(1500));
        let plain = strip_ansi_for_test(&line);
        assert!(plain.contains(WorkStatus::Working.localized_label()));
        assert!(plain.contains("1s"));
        assert!(!plain.contains("1.5s"));
        assert!(!plain.contains('·'));
        assert_ne!(
            line,
            WorkStatus::Working.render_line(1, Duration::from_millis(1500))
        );
    }

    /// 【终端】【工作状态测试】验证耗时格式不包含小数秒。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn elapsed_seconds_never_include_decimal_tenths() {
        assert_eq!(format_elapsed(Duration::from_millis(999)), "0s");
        assert_eq!(format_elapsed(Duration::from_millis(1500)), "1s");
        assert_eq!(format_elapsed(Duration::from_secs(65)), "1m05s");
    }

    /// 【终端】【工作状态测试】验证权限交互不会切换到 Working。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
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
