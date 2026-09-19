use super::WebEvent;
use crate::agent::AgentEvent;
use crate::llm::ChatStreamKind;
use crate::runner::RunnerEvent;
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};

/// 将 Sai 运行事件关联成稳定的浏览器消息与工具生命周期。
///
/// 生命周期与会话一致：同一会话的连续轮次共用同一个组装器，轮次边界由
/// `begin_run` 显式重置，因此新打开的标签页能从事件流重建每一轮的输入。
pub(crate) struct EventAssembler {
    run_id: String,
    workspace_id: String,
    session_id: String,
    /// 当前轮次的用户输入，供 run.started 事件回传给后加入的观察者
    run_input: String,
    run_image_urls: Vec<String>,
    status: Option<&'static str>,
    /// 是否已进入正文输出阶段；进入后不再回退到 thinking
    content_started: bool,
    tool_ids_by_index: HashMap<usize, String>,
    tool_ids_by_provider_id: HashMap<String, String>,
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
    /// 创建会话级运行事件组装器。
    ///
    /// 参数:
    /// - `workspace_id`: 工作区 ID
    /// - `session_id`: 会话 ID
    ///
    /// 返回:
    /// - 事件组装器
    pub(crate) fn new(workspace_id: &str, session_id: &str) -> Self {
        Self {
            run_id: String::new(),
            workspace_id: workspace_id.to_string(),
            session_id: session_id.to_string(),
            run_input: String::new(),
            run_image_urls: Vec::new(),
            status: None,
            content_started: false,
            tool_ids_by_index: HashMap::new(),
            tool_ids_by_provider_id: HashMap::new(),
            prepared_tools: VecDeque::new(),
            active_tools_by_name: HashMap::new(),
            next_tool_id: 0,
        }
    }

    /// 开启新一轮，重置轮次边界状态。
    ///
    /// 工具 ID 计数器保持单调递增，保证跨轮次不复用同一标识。
    ///
    /// 参数:
    /// - `run_id`: 运行 ID
    /// - `input`: 本轮用户输入
    /// - `image_urls`: 本轮图片列表
    ///
    /// 返回:
    /// - 无
    pub(crate) fn begin_run(&mut self, run_id: &str, input: &str, image_urls: &[String]) {
        self.run_id = run_id.to_string();
        self.run_input = input.to_string();
        self.run_image_urls = image_urls.to_vec();
        self.status = None;
        self.content_started = false;
        self.tool_ids_by_index.clear();
        self.tool_ids_by_provider_id.clear();
        self.prepared_tools.clear();
        self.active_tools_by_name.clear();
    }

    /// 转换单条 RunnerEvent。
    ///
    /// 参数:
    /// - `event`: Sai 运行事件
    ///
    /// 返回:
    /// - 一条或多条 Web 事件
    pub(crate) fn map(&mut self, event: RunnerEvent) -> Vec<WebEvent> {
        match event {
            RunnerEvent::Started => {
                let mut events = self.status_event("waiting_response");
                events.push(self.event(
                    "run.started",
                    json!({
                        "input": self.run_input,
                        "image_urls": self.run_image_urls,
                    }),
                ));
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
                        "ttft_ms": result.ttft_ms,
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
                    "ttft_ms": summary.last_turn_ttft_ms,
                }),
            )],
        }
    }

    /// 转换 AgentEvent。
    fn map_agent_event(&mut self, event: AgentEvent) -> Vec<WebEvent> {
        match event {
            AgentEvent::Chunk(chunk) => {
                let (status, kind) = match chunk.kind {
                    ChatStreamKind::Content => {
                        self.content_started = true;
                        ("working", "message.content.delta")
                    }
                    // 正文已开始后，晚到的推理增量不再把状态打回 thinking
                    ChatStreamKind::Reasoning if self.content_started => {
                        ("working", "message.reasoning.delta")
                    }
                    ChatStreamKind::Reasoning => ("thinking", "message.reasoning.delta"),
                };
                let mut events = self.status_event(status);
                events.push(self.event(kind, json!({ "text": chunk.text })));
                events
            }
            AgentEvent::InterMessage(message) => {
                let mut events = self.status_event("waiting_response");
                events.push(self.event(
                    "message.automatic.input",
                    json!({
                        "id": message.id,
                        "kind": message.kind.as_str(),
                        "content": message.content,
                    }),
                ));
                events
            }
            AgentEvent::WaitingExternal => self.status_event("waiting_external"),
            // Web：重连走 status.changed，并带上 attempt/max，供指示器展示进度
            AgentEvent::Reconnecting {
                attempt,
                max_attempts,
            } => {
                self.status = Some("reconnecting");
                vec![self.event(
                    "status.changed",
                    json!({
                        "status": "reconnecting",
                        "attempt": attempt,
                        "max_attempts": max_attempts,
                    }),
                )]
            }
            AgentEvent::ContextUpdated(update) => vec![self.event(
                "context.updated",
                json!({
                    "usage": update.usage,
                    "context_window_tokens": update.context_window_tokens,
                }),
            )],
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
            AgentEvent::ToolCallIdentified {
                id,
                name,
                arguments,
            } => {
                let mut events = self.status_event("working");
                let tool_id = self.tool_id_for_provider_call(&id);
                events.push(self.event(
                    "tool.call.started",
                    json!({ "tool_id": tool_id, "name": name, "arguments": arguments }),
                ));
                events
            }
            AgentEvent::ToolProgress { name, message } => {
                // SSH 秘密交互标记不当作普通进度渲染，转成独立的安全输入事件，
                // 避免标记文本进入工具进度流；标记本身不含任何秘密
                if crate::ssh::is_secret_marker(&message) {
                    return self.map_ssh_secret_marker(&message);
                }
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
            AgentEvent::ToolProgressIdentified { id, name, message } => {
                if crate::ssh::is_secret_marker(&message) {
                    return self.map_ssh_secret_marker(&message);
                }
                let mut events = self.status_event("working");
                let tool_id = self.tool_id_for_provider_call(&id);
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
            AgentEvent::ToolResultIdentified {
                id,
                name,
                ok,
                output,
            } => {
                let mut events = self.status_event("working");
                let tool_id = self.tool_id_for_provider_call(&id);
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
            AgentEvent::EngineReady { engine, version } => vec![self.event(
                "engine.ready",
                json!({ "engine": engine, "version": version }),
            )],
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

    /// 将 SSH 秘密交互带外标记转换为浏览器可消费的安全输入事件。
    ///
    /// 请求标记携带无秘密的征询元信息，供前端弹出安全输入界面；结束标记通知前端
    /// 收起界面。真正的秘密由前端经专用提交端点直达后端，不经此事件流。
    ///
    /// 参数:
    /// - `message`: 工具进度中的带外标记
    ///
    /// 返回:
    /// - 对应的安全输入事件
    fn map_ssh_secret_marker(&mut self, message: &str) -> Vec<WebEvent> {
        if let Some(request) = crate::ssh::decode_progress_marker(message) {
            let mut events = self.status_event("waiting_ssh_secret");
            let payload = serde_json::to_value(&request).unwrap_or_else(|_| json!({}));
            events.push(self.event("ssh.secret.requested", payload));
            return events;
        }
        if let Some(request_id) = crate::ssh::decode_resolved_marker(message) {
            let mut events = self.status_event("working");
            events.push(self.event("ssh.secret.resolved", json!({ "request_id": request_id })));
            return events;
        }
        Vec::new()
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

    /// 返回 provider 调用标识对应的稳定 Web 工具标识。
    ///
    /// 参数:
    /// - `provider_id`: provider 在工具生命周期中使用的调用标识
    ///
    /// 返回:
    /// - 当前运行内稳定且唯一的 Web 工具标识
    fn tool_id_for_provider_call(&mut self, provider_id: &str) -> String {
        if let Some(id) = self.tool_ids_by_provider_id.get(provider_id) {
            return id.clone();
        }
        let id = self.allocate_tool_id();
        self.tool_ids_by_provider_id
            .insert(provider_id.to_string(), id.clone());
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
        "edit_file" | "run_command" | "background_command" | "trash_path" | "subagent"
    )
}

#[cfg(test)]
mod tests;
