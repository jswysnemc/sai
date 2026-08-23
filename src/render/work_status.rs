use crate::agent::AgentEvent;
use crate::llm::ChatStreamKind;
use crate::render::activity_animation::render_activity_line;
use std::time::Duration;

/// 单轮请求的用户可见工作状态。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkStatus {
    /// 已发出请求，等待模型首包
    WaitingResponse,
    /// 等待后台/外部工作完成后继续
    WaitingExternal,
    /// 等待工具开始执行
    WaitingToRun,
    /// 等待写入磁盘（编辑类工具）
    WaitingToWrite,
    Thinking,
    Working,
    Compacting,
    /// 传输层瞬时故障后的自动重连，带当前尝试次数。
    Reconnecting {
        attempt: u32,
        max_attempts: u32,
    },
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
            AgentEvent::Chunk(_) => Some(Self::Working),
            AgentEvent::InterMessage(_)
            | AgentEvent::ToolResult { .. }
            | AgentEvent::ToolResultIdentified { .. }
            | AgentEvent::PermissionResolved { .. }
            | AgentEvent::QuestionResolved { .. } => Some(Self::WaitingResponse),
            AgentEvent::ToolCall { name, .. }
            | AgentEvent::ToolCallIdentified { name, .. }
            | AgentEvent::ToolProgress { name, .. }
            | AgentEvent::ToolProgressIdentified { name, .. } => Some(status_for_tool(name)),
            AgentEvent::ToolCallProgress(progress) => Some(
                progress
                    .name
                    .as_deref()
                    .map(status_for_tool)
                    .unwrap_or(Self::WaitingToRun),
            ),
            // 权限/提问交互期间由专门 UI 接管，不进入 Working，避免与审核行重叠
            AgentEvent::WaitingExternal => Some(Self::WaitingExternal),
            AgentEvent::Reconnecting {
                attempt,
                max_attempts,
            } => Some(Self::Reconnecting {
                attempt: *attempt,
                max_attempts: *max_attempts,
            }),
            AgentEvent::PermissionRequested(_) | AgentEvent::QuestionRequested(_) => None,
            AgentEvent::CompactionStarted { .. } => Some(Self::Compacting),
            AgentEvent::CompactionDelta { .. }
            | AgentEvent::CompactionFinished { .. }
            | AgentEvent::ContextUpdated(_)
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
    pub(crate) fn label(self) -> String {
        match self {
            Self::WaitingResponse => "Waiting for response".to_string(),
            Self::WaitingExternal | Self::WaitingToRun => "Waiting to run".to_string(),
            Self::WaitingToWrite => "Waiting to write".to_string(),
            Self::Thinking => "Thinking".to_string(),
            Self::Working => "Working".to_string(),
            Self::Compacting => "Compacting".to_string(),
            Self::Reconnecting {
                attempt,
                max_attempts,
            } => format!("Reconnecting... {attempt}/{max_attempts}"),
        }
    }

    /// 【终端】【工作状态】返回动效状态文案。
    ///
    /// TUI/CLI 与 Codex 一致：重连只改状态行动效文案（`Reconnecting... N/M`），
    /// 不写入历史 cell，避免瞬时断连刷屏。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 英文状态短语；重连状态带上当前尝试次数
    pub(crate) fn localized_label(self) -> String {
        match self {
            Self::WaitingResponse => "Waiting for response".to_string(),
            Self::WaitingExternal | Self::WaitingToRun => "Waiting to run".to_string(),
            Self::WaitingToWrite => "Waiting to write".to_string(),
            Self::Thinking => "Thinking".to_string(),
            Self::Working => "Working".to_string(),
            Self::Compacting => "Compacting".to_string(),
            Self::Reconnecting {
                attempt,
                max_attempts,
            } => format!("Reconnecting... {attempt}/{max_attempts}"),
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
        let label = self.localized_label();
        render_activity_line(&label, &format_elapsed(elapsed), frame)
    }
}

/// 按工具种类选择等待语义。
///
/// 参数:
/// - `name`: 工具名称
///
/// 返回:
/// - 编辑类为等待写入，其余为等待运行
pub(crate) fn status_for_tool(name: &str) -> WorkStatus {
    if crate::render::stream_text::is_file_edit_tool(name) {
        WorkStatus::WaitingToWrite
    } else {
        WorkStatus::WaitingToRun
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
        format!("{mins}m {secs:02}s")
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

    /// 【终端】【工作状态】验证工具事件使用等待运行 / 等待写入，结果后回到等待响应。
    #[test]
    fn tool_events_use_waiting_semantics() {
        let run = AgentEvent::ToolCall {
            name: "read_file".into(),
            arguments: "{}".into(),
        };
        let write = AgentEvent::ToolCall {
            name: "str_replace".into(),
            arguments: "{}".into(),
        };
        let done = AgentEvent::ToolResult {
            name: "read_file".into(),
            ok: true,
            output: "ok".into(),
        };
        assert_eq!(
            WorkStatus::from_agent_event(&run),
            Some(WorkStatus::WaitingToRun)
        );
        assert_eq!(
            WorkStatus::from_agent_event(&write),
            Some(WorkStatus::WaitingToWrite)
        );
        assert_eq!(
            WorkStatus::from_agent_event(&done),
            Some(WorkStatus::WaitingResponse)
        );
        assert_eq!(WorkStatus::WaitingToRun.localized_label(), "Waiting to run");
        assert_eq!(
            WorkStatus::WaitingToWrite.localized_label(),
            "Waiting to write"
        );
        assert_eq!(
            WorkStatus::WaitingResponse.localized_label(),
            "Waiting for response"
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
        assert!(plain.contains(&WorkStatus::Working.localized_label()));
        assert!(plain.contains("1s"));
        assert!(!plain.contains("1.5s"));
        assert!(!plain.contains('·'));
        // 亮带按字符位离散推进，相邻帧可能停在原位；跨过一个字符位再比较
        assert_ne!(
            line,
            WorkStatus::Working.render_line(14, Duration::from_millis(1500))
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
        assert_eq!(format_elapsed(Duration::from_secs(65)), "1m 05s");
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

    /// 【终端】【重连状态】验证传输重连映射为带次数的状态行文案。
    #[test]
    fn reconnecting_maps_to_codex_style_status_label() {
        let event = AgentEvent::Reconnecting {
            attempt: 2,
            max_attempts: 3,
        };
        assert_eq!(
            WorkStatus::from_agent_event(&event),
            Some(WorkStatus::Reconnecting {
                attempt: 2,
                max_attempts: 3
            })
        );
        assert_eq!(
            WorkStatus::Reconnecting {
                attempt: 2,
                max_attempts: 3
            }
            .localized_label(),
            "Reconnecting... 2/3"
        );
    }
}
