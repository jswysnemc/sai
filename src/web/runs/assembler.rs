use super::WebEvent;
use crate::agent::AgentEvent;
use crate::llm::ChatStreamKind;
use crate::runner::RunnerEvent;
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};

/// 将 Sai 运行事件关联成稳定的浏览器消息与工具生命周期。
pub(super) struct EventAssembler {
    run_id: String,
    workspace_id: String,
    session_id: String,
    status: Option<&'static str>,
    tool_ids_by_index: HashMap<usize, String>,
    prepared_tools: VecDeque<PreparedTool>,
    active_tools_by_name: HashMap<String, VecDeque<String>>,
    next_tool_id: usize,
}

/// 流式参数阶段观察到的工具调用。
struct PreparedTool {
    index: usize,
    name: Option<String>,
}

impl EventAssembler {
    /// 创建运行事件组装器。
    ///
    /// 参数:
    /// - `run_id`: 运行 ID
    /// - `workspace_id`: 工作区 ID
    /// - `session_id`: 会话 ID
    ///
    /// 返回:
    /// - 事件组装器
    pub(super) fn new(run_id: &str, workspace_id: &str, session_id: &str) -> Self {
        Self {
            run_id: run_id.to_string(),
            workspace_id: workspace_id.to_string(),
            session_id: session_id.to_string(),
            status: None,
            tool_ids_by_index: HashMap::new(),
            prepared_tools: VecDeque::new(),
            active_tools_by_name: HashMap::new(),
            next_tool_id: 0,
        }
    }

    /// 转换单条 RunnerEvent。
    ///
    /// 参数:
    /// - `event`: Sai 运行事件
    ///
    /// 返回:
    /// - 一条或多条 Web 事件
    pub(super) fn map(&mut self, event: RunnerEvent) -> Vec<WebEvent> {
        match event {
            RunnerEvent::Started => {
                let mut events = self.status_event("waiting_response");
                events.push(self.event("run.started", json!({})));
                events
            }
            RunnerEvent::AutomaticInput(input) => {
                let mut events = self.status_event("waiting_response");
                events.push(self.event(
                    "message.automatic.input",
                    json!({
                        "kind": input.kind.as_str(),
                        "content": input.content,
                    }),
                ));
                events
            }
            RunnerEvent::WaitingExternal => self.status_event("waiting_external"),
            RunnerEvent::Agent(event) => self.map_agent_event(event),
            RunnerEvent::Interrupted => {
                self.status = None;
                vec![self.event(
                    "run.interrupted",
                    json!({
                        "detail": "The runner stopped before it produced a terminal response."
                    }),
                )]
            }
            RunnerEvent::Completed(result) => {
                self.status = None;
                vec![self.event(
                    "run.completed",
                    json!({
                        "content": result.content,
                        "reasoning": result.reasoning,
                        "usage": result.usage,
                        "duration_ms": result.duration_ms,
                    }),
                )]
            }
            RunnerEvent::Failed(message) => {
                self.status = None;
                vec![self.event(
                    "run.failed",
                    json!({ "message": message, "detail": message }),
                )]
            }
            RunnerEvent::LoadedToolsChanged(tools) => {
                vec![self.event("loaded_tools.changed", json!({ "tools": tools }))]
            }
            RunnerEvent::FinalSummary(summary) => vec![self.event(
                "session.summary",
                json!({
                    "session_id": summary.session_id,
                    "turn_count": summary.turn_count,
                    "context_chars": summary.context_chars,
                    "context_limit_chars": summary.context_limit_chars,
                    "context_ratio": summary.context_ratio,
                    "context_prompt_tokens": summary.context_prompt_tokens,
                    "context_window_tokens": summary.context_window_tokens,
                    "context_token_ratio": summary.context_token_ratio,
                    "duration_ms": summary.last_turn_duration_ms,
                }),
            )],
        }
    }

    /// 转换 AgentEvent。
    fn map_agent_event(&mut self, event: AgentEvent) -> Vec<WebEvent> {
        match event {
            AgentEvent::Chunk(chunk) => {
                let (status, kind) = match chunk.kind {
                    ChatStreamKind::Content => ("working", "message.content.delta"),
                    ChatStreamKind::Reasoning => ("thinking", "message.reasoning.delta"),
                };
                let mut events = self.status_event(status);
                events.push(self.event(kind, json!({ "text": chunk.text })));
                events
            }
            AgentEvent::ToolCallProgress(progress) => {
                let mut events = self.status_event("working");
                let tool_id = self.tool_id_for_index(progress.index);
                // 1. 记录或更新该流式索引对应的调用名称，供正式调用按名称配对
                if let Some(entry) = self
                    .prepared_tools
                    .iter_mut()
                    .find(|entry| entry.index == progress.index)
                {
                    if entry.name.is_none() {
                        entry.name = progress.name.clone();
                    }
                } else {
                    self.prepared_tools.push_back(PreparedTool {
                        index: progress.index,
                        name: progress.name.clone(),
                    });
                }
                events.push(self.event(
                    "tool.call.preparing",
                    json!({
                        "tool_id": tool_id,
                        "index": progress.index,
                        "name": progress.name,
                        "arguments_chars": progress.arguments_chars,
                        "arguments_bytes": progress.arguments_bytes,
                        "arguments_preview": progress.arguments_preview,
                    }),
                ));
                events
            }
            AgentEvent::ToolCall { name, arguments } => {
                let mut events = self.status_event("working");
                let tool_id = self.tool_id_for_next_call(&name);
                self.active_tools_by_name
                    .entry(name.clone())
                    .or_default()
                    .push_back(tool_id.clone());
                events.push(self.event(
                    "tool.call.started",
                    json!({ "tool_id": tool_id, "name": name, "arguments": arguments }),
                ));
                events
            }
            AgentEvent::ToolProgress { name, message } => {
                let mut events = self.status_event("working");
                if name == "run_command"
                    && crate::tools::command::decode_command_output(&message).is_some()
                {
                    return events;
                }
                let tool_id = self.active_tool_id(&name);
                events.push(self.event(
                    "tool.progress",
                    json!({ "tool_id": tool_id, "name": name, "message": message }),
                ));
                events
            }
            AgentEvent::ToolResult { name, ok, output } => {
                let mut events = self.status_event("working");
                let tool_id = self.finish_tool_id(&name);
                events.push(self.event(
                    "tool.result",
                    json!({ "tool_id": tool_id, "name": name, "ok": ok, "output": output }),
                ));
                if ok && tool_can_mutate_workspace(&name) {
                    events.push(self.event(
                        "workspace.changed",
                        json!({ "source": "tool", "tool_id": tool_id, "tool_name": name }),
                    ));
                }
                events
            }
            AgentEvent::PermissionRequested(request) => {
                let mut events = self.status_event("waiting_permission");
                events.push(self.event("permission.requested", json!(request)));
                events
            }
            AgentEvent::PermissionResolved {
                request_id,
                decision,
            } => {
                let mut events = self.status_event("working");
                events.push(self.event(
                    "permission.resolved",
                    json!({ "request_id": request_id, "decision": decision }),
                ));
                events
            }
            AgentEvent::QuestionRequested(pending) => {
                let mut events = self.status_event("waiting_question");
                events.push(self.event("question.requested", json!(pending)));
                events
            }
            AgentEvent::QuestionResolved {
                request_id,
                response,
            } => {
                let mut events = self.status_event("working");
                events.push(self.event(
                    "question.resolved",
                    json!({ "request_id": request_id, "response": response }),
                ));
                events
            }
            AgentEvent::CompactionStarted { turn_count, model } => {
                let mut events = self.status_event("compacting");
                events.push(self.event(
                    "compaction.started",
                    json!({ "turn_count": turn_count, "model": model }),
                ));
                events
            }
            AgentEvent::CompactionDelta { text } => {
                vec![self.event("compaction.delta", json!({ "text": text }))]
            }
            AgentEvent::CompactionFinished {
                applied,
                summary,
                error,
            } => {
                let error =
                    error.map(|error| json!({ "message": error.message, "detail": error.detail }));
                vec![self.event(
                    "compaction.finished",
                    json!({ "applied": applied, "summary": summary, "error": error }),
                )]
            }
            AgentEvent::FlushContent => vec![self.event("content.flushed", json!({}))],
            AgentEvent::ExternalOutput => vec![self.event("external.output", json!({}))],
        }
    }

    /// 仅在工作状态变化时生成事件。
    fn status_event(&mut self, status: &'static str) -> Vec<WebEvent> {
        if self.status == Some(status) {
            return Vec::new();
        }
        self.status = Some(status);
        vec![self.event("status.changed", json!({ "status": status }))]
    }

    /// 返回指定流式索引的稳定工具 ID。
    fn tool_id_for_index(&mut self, index: usize) -> String {
        if let Some(id) = self.tool_ids_by_index.get(&index) {
            return id.clone();
        }
        let id = self.allocate_tool_id();
        self.tool_ids_by_index.insert(index, id.clone());
        id
    }

    /// 返回下一正式工具调用的 ID。
    ///
    /// 参数:
    /// - `name`: 正式调用的工具名称
    ///
    /// 返回:
    /// - 与 preparing 阶段对齐的稳定工具 ID
    fn tool_id_for_next_call(&mut self, name: &str) -> String {
        // 1. 优先匹配名称一致的 preparing 条目，避免供应商丢弃无名调用导致队列错位
        let position = self
            .prepared_tools
            .iter()
            .position(|entry| entry.name.as_deref() == Some(name))
            // 2. 找不到同名条目时退回第一个未知名称的条目
            .or_else(|| {
                self.prepared_tools
                    .iter()
                    .position(|entry| entry.name.is_none())
            });
        if let Some(position) = position {
            let entry = self.prepared_tools.remove(position);
            if let Some(entry) = entry {
                let id = self.tool_id_for_index(entry.index);
                self.tool_ids_by_index.remove(&entry.index);
                return id;
            }
        }
        // 3. 完全没有 preparing 记录时分配新 ID
        self.allocate_tool_id()
    }

    /// 分配运行内全局唯一的工具 ID。
    ///
    /// 返回:
    /// - 新工具 ID
    fn allocate_tool_id(&mut self) -> String {
        let sequence = self.next_tool_id;
        self.next_tool_id = self.next_tool_id.saturating_add(1);
        format!("{}-tool-{sequence}", self.run_id)
    }

    /// 返回指定名称当前活动工具 ID。
    fn active_tool_id(&self, name: &str) -> String {
        self.active_tools_by_name
            .get(name)
            .and_then(|ids| ids.front())
            .cloned()
            .unwrap_or_else(|| format!("{}-tool-unknown", self.run_id))
    }

    /// 结束并移除指定名称的活动工具 ID。
    fn finish_tool_id(&mut self, name: &str) -> String {
        let id = self
            .active_tools_by_name
            .get_mut(name)
            .and_then(VecDeque::pop_front)
            .unwrap_or_else(|| format!("{}-tool-unknown", self.run_id));
        if self
            .active_tools_by_name
            .get(name)
            .is_some_and(VecDeque::is_empty)
        {
            self.active_tools_by_name.remove(name);
        }
        id
    }

    /// 创建当前运行上下文中的 Web 事件。
    fn event(&self, kind: &str, payload: Value) -> WebEvent {
        WebEvent::new(
            &self.run_id,
            &self.workspace_id,
            &self.session_id,
            kind,
            payload,
        )
    }
}

/// 判断工具执行成功后是否可能修改了工作区文件。
///
/// 参数:
/// - `name`: 工具名称
///
/// 返回:
/// - 是否需要通知前端刷新文件树与差异视图
fn tool_can_mutate_workspace(name: &str) -> bool {
    matches!(
        name,
        "edit_file"
            | "apply_patch"
            | "write_file"
            | "replace_file_lines"
            | "run_command"
            | "background_command"
            | "trash_path"
            | "subagent"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::ToolCallStreamProgress;
    use crate::runner::{AutomaticInputEvent, AutomaticInputKind};

    /// 验证运行中断事件包含可供前端展开的诊断详情。
    #[test]
    fn interrupted_event_contains_diagnostic_detail() {
        let mut assembler = EventAssembler::new("run", "workspace", "session");

        let events = assembler.map(RunnerEvent::Interrupted);
        let interrupted = events
            .iter()
            .find(|event| event.kind == "run.interrupted")
            .unwrap();

        assert!(interrupted.payload["detail"]
            .as_str()
            .is_some_and(|detail| !detail.trim().is_empty()));
    }

    /// 验证权限决定会作为可重放事件发送到 Web 消息流。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn emits_permission_resolution_for_stream_replay() {
        let mut assembler = EventAssembler::new("run", "workspace", "session");

        let events = assembler.map(RunnerEvent::Agent(AgentEvent::PermissionResolved {
            request_id: "permission".to_string(),
            decision: crate::permission::PermissionDecision::Deny {
                reply: Some("保留文件".to_string()),
            },
        }));
        let resolved = events
            .iter()
            .find(|event| event.kind == "permission.resolved")
            .unwrap();

        assert_eq!(resolved.payload["request_id"], "permission");
        assert_eq!(resolved.payload["decision"]["decision"], "deny");
        assert_eq!(resolved.payload["decision"]["reply"], "保留文件");
    }

    #[test]
    fn keeps_tool_progress_and_result_on_one_tool_id() {
        let mut assembler = EventAssembler::new("run", "workspace", "session");
        let preparing = assembler.map(RunnerEvent::Agent(AgentEvent::ToolCallProgress(
            ToolCallStreamProgress {
                index: 0,
                name: Some("edit_file".to_string()),
                arguments_chars: 10,
                arguments_bytes: 10,
                arguments_preview: "{\"patch\":".to_string(),
            },
        )));
        let started = assembler.map(RunnerEvent::Agent(AgentEvent::ToolCall {
            name: "edit_file".to_string(),
            arguments: "{}".to_string(),
        }));
        let result = assembler.map(RunnerEvent::Agent(AgentEvent::ToolResult {
            name: "edit_file".to_string(),
            ok: true,
            output: "ok".to_string(),
        }));
        let id = preparing
            .iter()
            .find(|event| event.kind == "tool.call.preparing")
            .unwrap()
            .payload["tool_id"]
            .as_str()
            .unwrap();
        assert_eq!(started.last().unwrap().payload["tool_id"], id);
        assert_eq!(result.last().unwrap().payload["tool_id"], id);
    }

    #[test]
    fn hides_internal_command_output_progress_events() {
        let mut assembler = EventAssembler::new("run", "workspace", "session");
        let message = crate::tools::command::encode_command_output_for_test(
            crate::tools::command::CommandOutputStream::Stdout,
            b"building\n",
        );
        let events = assembler.map(RunnerEvent::Agent(AgentEvent::ToolProgress {
            name: "run_command".to_string(),
            message,
        }));

        assert!(events.iter().all(|event| event.kind != "tool.progress"));
    }

    #[test]
    fn emits_status_only_when_it_changes() {
        let mut assembler = EventAssembler::new("run", "workspace", "session");
        let first = assembler.map(RunnerEvent::Agent(AgentEvent::Chunk(
            crate::llm::ChatStreamChunk {
                kind: ChatStreamKind::Content,
                text: "a".to_string(),
            },
        )));
        let second = assembler.map(RunnerEvent::Agent(AgentEvent::Chunk(
            crate::llm::ChatStreamChunk {
                kind: ChatStreamKind::Content,
                text: "b".to_string(),
            },
        )));
        assert_eq!(
            first
                .iter()
                .filter(|event| event.kind == "status.changed")
                .count(),
            1
        );
        assert_eq!(
            second
                .iter()
                .filter(|event| event.kind == "status.changed")
                .count(),
            0
        );
    }

    #[test]
    fn allocates_new_id_when_provider_index_restarts_next_round() {
        let mut assembler = EventAssembler::new("run", "workspace", "session");
        let first = assembler.map(RunnerEvent::Agent(AgentEvent::ToolCallProgress(
            ToolCallStreamProgress {
                index: 0,
                name: Some("edit_file".to_string()),
                arguments_chars: 10,
                arguments_bytes: 10,
                arguments_preview: "{}".to_string(),
            },
        )));
        assembler.map(RunnerEvent::Agent(AgentEvent::ToolCall {
            name: "edit_file".to_string(),
            arguments: "{}".to_string(),
        }));
        assembler.map(RunnerEvent::Agent(AgentEvent::ToolResult {
            name: "edit_file".to_string(),
            ok: true,
            output: "ok".to_string(),
        }));
        let second = assembler.map(RunnerEvent::Agent(AgentEvent::ToolCallProgress(
            ToolCallStreamProgress {
                index: 0,
                name: Some("read_file".to_string()),
                arguments_chars: 10,
                arguments_bytes: 10,
                arguments_preview: "{}".to_string(),
            },
        )));
        let first_id = first.last().unwrap().payload["tool_id"].as_str().unwrap();
        let second_id = second.last().unwrap().payload["tool_id"].as_str().unwrap();
        assert_ne!(first_id, second_id);
    }

    #[test]
    fn pairs_started_by_name_when_unnamed_call_is_dropped() {
        let mut assembler = EventAssembler::new("run", "workspace", "session");
        // 1. 幻影调用只出现在流式阶段且没有名称，最终会被供应商丢弃
        assembler.map(RunnerEvent::Agent(AgentEvent::ToolCallProgress(
            ToolCallStreamProgress {
                index: 0,
                name: None,
                arguments_chars: 0,
                arguments_bytes: 1,
                arguments_preview: String::new(),
            },
        )));
        let edit_preparing = assembler.map(RunnerEvent::Agent(AgentEvent::ToolCallProgress(
            ToolCallStreamProgress {
                index: 1,
                name: Some("edit_file".to_string()),
                arguments_chars: 10,
                arguments_bytes: 10,
                arguments_preview: "{\"patch\":".to_string(),
            },
        )));
        let edit_started = assembler.map(RunnerEvent::Agent(AgentEvent::ToolCall {
            name: "edit_file".to_string(),
            arguments: "{}".to_string(),
        }));
        let prepared_id = edit_preparing.last().unwrap().payload["tool_id"]
            .as_str()
            .unwrap();
        let started_id = edit_started.last().unwrap().payload["tool_id"]
            .as_str()
            .unwrap();
        assert_eq!(prepared_id, started_id);
    }

    #[test]
    fn emits_workspace_change_after_successful_edit() {
        let mut assembler = EventAssembler::new("run", "workspace", "session");
        assembler.map(RunnerEvent::Agent(AgentEvent::ToolCall {
            name: "edit_file".to_string(),
            arguments: "{}".to_string(),
        }));
        let events = assembler.map(RunnerEvent::Agent(AgentEvent::ToolResult {
            name: "edit_file".to_string(),
            ok: true,
            output: "ok".to_string(),
        }));
        assert!(events.iter().any(|event| event.kind == "workspace.changed"));
    }

    /// 验证 Goal 等待外部工作时向 Web 暴露独立状态。
    #[test]
    fn maps_external_waiting_status() {
        let mut assembler = EventAssembler::new("run", "workspace", "session");
        let events = assembler.map(RunnerEvent::WaitingExternal);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, "status.changed");
        assert_eq!(events[0].payload["status"], "waiting_external");
    }

    /// 验证自动输入事件向 Web 传递展示文本而不是内部提示。
    #[test]
    fn maps_automatic_input_message() {
        let mut assembler = EventAssembler::new("run", "workspace", "session");
        let events = assembler.map(RunnerEvent::AutomaticInput(AutomaticInputEvent::new(
            AutomaticInputKind::ExternalCompletion,
            "后台任务已完成".to_string(),
        )));

        let message = events
            .iter()
            .find(|event| event.kind == "message.automatic.input")
            .unwrap();
        assert_eq!(message.payload["kind"], "external_completion");
        assert_eq!(message.payload["content"], "后台任务已完成");
    }
}
