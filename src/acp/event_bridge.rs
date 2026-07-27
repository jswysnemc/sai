use crate::agent::AgentEvent;
use crate::llm::{ChatStreamChunk, ChatStreamKind, Usage};
use agent_client_protocol::schema::v1::{SessionConfigOption, SessionUpdate};
use serde_json::Value;
use std::collections::HashMap;

/// 一条 `session/update` 翻译出的结果。
#[derive(Debug, Default)]
pub(crate) struct BridgedUpdate {
    /// 需要转发给 UI 的事件
    pub(crate) events: Vec<AgentEvent>,
    /// 助手正文增量，用于累计本轮回复
    pub(crate) content_delta: String,
    /// 思考增量，用于累计本轮推理
    pub(crate) reasoning_delta: String,
    /// agent 报告的当前上下文用量
    pub(crate) usage: Option<Usage>,
    /// agent 推送的完整 session configOptions 状态
    pub(crate) config_options: Option<Vec<SessionConfigOption>>,
    /// agent 推送的旧版当前模式
    pub(crate) current_mode: Option<String>,
}

/// ACP 会话更新桥接器。
///
/// 工具更新只保证携带 `toolCallId`，后续状态可能省略标题与类型。
/// 这里保存当前会话中的工具名称，使所有阶段都能使用同一名称展示。
#[derive(Debug, Default)]
pub(crate) struct AcpEventBridge {
    tool_names: HashMap<String, String>,
}

impl AcpEventBridge {
    /// 把 `session/update` 通知翻译成 sai 的事件模型。
    ///
    /// ACP 的更新类型与 `AgentEvent` 几乎一一对应，因此三端 UI 不需要区分内核。
    /// 无法对应的类型静默忽略：协议仍在演进，未知更新不应中断对话。
    ///
    /// 参数:
    /// - `params`: `session/update` 的参数
    ///
    /// 返回:
    /// - 翻译结果
    pub(crate) fn bridge_session_update(&mut self, params: &Value) -> BridgedUpdate {
        let mut bridged = BridgedUpdate::default();
        let Some(update) = params.get("update") else {
            return bridged;
        };
        // 官方 SDK 负责已知更新的结构校验；厂商扩展仍由下方宽容解析保留
        let typed_update =
            super::sdk::from_value::<SessionUpdate>(update.clone(), "session/update notification")
                .ok();
        let kind = update
            .get("sessionUpdate")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match kind {
            "agent_message_chunk" => {
                let text = content_text(update.get("content"));
                if !text.is_empty() {
                    bridged.content_delta.push_str(&text);
                    bridged.events.push(AgentEvent::Chunk(ChatStreamChunk {
                        kind: ChatStreamKind::Content,
                        text,
                    }));
                }
            }
            "agent_thought_chunk" => {
                let text = content_text(update.get("content"));
                if !text.is_empty() {
                    bridged.reasoning_delta.push_str(&text);
                    bridged.events.push(AgentEvent::Chunk(ChatStreamChunk {
                        kind: ChatStreamKind::Reasoning,
                        text,
                    }));
                }
            }
            "tool_call" => self.bridge_tool_call(update, &mut bridged),
            "tool_call_update" => self.bridge_tool_update(update, &mut bridged),
            // plan 对应 sai 的待办清单：作为工具结果送进去，复用既有的清单渲染
            "plan" => {
                if let Some(entries) = update.get("entries").and_then(Value::as_array) {
                    bridged.events.push(AgentEvent::ToolResult {
                        name: "todo".to_string(),
                        ok: true,
                        output: plan_to_todo_snapshot(entries),
                    });
                }
            }
            // 外部内核声明的斜杠命令：列出来让用户知道有什么可用，
            // 命令本身作为普通输入发给内核，由它自己解析
            "available_commands_update" => {
                if let Some(names) = command_names(update) {
                    bridged.events.push(AgentEvent::ToolProgress {
                        name: crate::i18n::text("engine commands", "内核命令").to_string(),
                        message: names,
                    });
                }
            }
            "config_option_update" => {
                if let Some(SessionUpdate::ConfigOptionUpdate(update)) = typed_update {
                    bridged.config_options = Some(update.config_options);
                }
            }
            "current_mode_update" => {
                if let Some(SessionUpdate::CurrentModeUpdate(update)) = typed_update {
                    bridged.current_mode = Some(update.current_mode_id.to_string());
                }
            }
            "usage_update" => {
                if let Some(SessionUpdate::UsageUpdate(update)) = typed_update {
                    bridged.usage = Some(Usage {
                        prompt_tokens: update.used,
                        completion_tokens: 0,
                        total_tokens: update.used,
                        cache_read_tokens: 0,
                        cache_write_tokens: 0,
                    });
                }
            }
            _ => {}
        }
        bridged
    }

    /// 翻译工具调用开始，并处理首次事件已经是终态的情况。
    ///
    /// 参数:
    /// - `update`: ACP 工具调用更新
    /// - `bridged`: 当前通知的翻译结果
    ///
    /// 返回:
    /// - 无
    fn bridge_tool_call(&mut self, update: &Value, bridged: &mut BridgedUpdate) {
        let id = tool_call_id(update);
        let name = tool_name(update);
        self.tool_names.insert(id.clone(), name.clone());
        let mut arguments = update
            .get("rawInput")
            .map(value_to_arguments)
            .unwrap_or_else(|| "{}".to_string());
        if let Some(locations) = update.get("locations") {
            arguments = merge_acp_metadata(&arguments, "locations", locations.clone());
        }
        bridged.events.push(AgentEvent::ToolCallIdentified {
            id: id.clone(),
            name: name.clone(),
            arguments,
        });
        if let Some(ok) = terminal_tool_status(update) {
            bridged.events.push(AgentEvent::ToolResultIdentified {
                id,
                name,
                ok,
                output: tool_output_text(update),
            });
        } else {
            let output = tool_output_text(update);
            if !output.is_empty() {
                bridged.events.push(AgentEvent::ToolProgressIdentified {
                    id,
                    name,
                    message: output,
                });
            }
        }
    }

    /// 翻译工具调用的后续状态，并通过调用标识恢复稳定名称。
    ///
    /// 参数:
    /// - `update`: ACP 工具状态更新
    /// - `bridged`: 当前通知的翻译结果
    ///
    /// 返回:
    /// - 无
    fn bridge_tool_update(&mut self, update: &Value, bridged: &mut BridgedUpdate) {
        let id = tool_call_id(update);
        let name = self
            .tool_names
            .get(&id)
            .cloned()
            .unwrap_or_else(|| tool_name(update));
        let mut output = tool_output_text(update);
        if let Some(locations) = update.get("locations") {
            output = merge_acp_metadata(&output, "locations", locations.clone());
        }
        if let Some(ok) = terminal_tool_status(update) {
            bridged.events.push(AgentEvent::ToolResultIdentified {
                id,
                name,
                ok,
                output,
            });
        } else if !output.is_empty() {
            bridged.events.push(AgentEvent::ToolProgressIdentified {
                id,
                name,
                message: output,
            });
        }
    }
}

/// 提取内核声明的可用命令名。
///
/// 参数:
/// - `update`: 更新对象
///
/// 返回:
/// - 以空格分隔的 `/命令` 列表；没有命令时返回 None
fn command_names(update: &Value) -> Option<String> {
    let commands = update.get("availableCommands")?.as_array()?;
    let names = commands
        .iter()
        .filter_map(|command| command.get("name").and_then(Value::as_str))
        .map(|name| format!("/{name}"))
        .collect::<Vec<_>>();
    (!names.is_empty()).then(|| names.join(" "))
}

/// 读取工具状态是否已经结束。
///
/// 参数:
/// - `update`: 更新对象
///
/// 返回:
/// - 成功结束为 `Some(true)`，失败结束为 `Some(false)`，进行中为 `None`
fn terminal_tool_status(update: &Value) -> Option<bool> {
    match update.get("status").and_then(Value::as_str) {
        Some("completed") => Some(true),
        Some("failed") => Some(false),
        _ => None,
    }
}

/// 提取 ACP 工具调用的稳定标识。
///
/// 参数:
/// - `update`: 工具更新对象
///
/// 返回:
/// - provider 工具调用标识；协议载荷损坏时使用稳定回退值
fn tool_call_id(update: &Value) -> String {
    update
        .get("toolCallId")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("tool")
        .to_string()
}

/// 提取工具名称。
///
/// ACP 用 `title` 描述人类可读的操作，`kind` 是粗分类；
/// sai 的工具卡按名称选渲染器，因此优先取更具体的 title。
///
/// 参数:
/// - `update`: 更新对象
///
/// 返回:
/// - 工具名称
fn tool_name(update: &Value) -> String {
    for key in ["title", "kind", "toolCallId"] {
        if let Some(value) = update.get(key).and_then(Value::as_str) {
            if !value.trim().is_empty() {
                return value.to_string();
            }
        }
    }
    "tool".to_string()
}

/// 从 content block 或其数组中取出纯文本。
///
/// 参数:
/// - `content`: content 字段
///
/// 返回:
/// - 拼接后的文本
fn content_text(content: Option<&Value>) -> String {
    let Some(content) = content else {
        return String::new();
    };
    match content {
        Value::String(text) => text.clone(),
        Value::Array(items) => items
            .iter()
            .map(|item| content_text(Some(item)))
            .collect::<Vec<_>>()
            .join(""),
        Value::Object(object) => {
            // 嵌套一层的 { type: "content", content: {...} } 同样要能取到文本
            if let Some(inner) = object.get("content") {
                return content_text(Some(inner));
            }
            object
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        }
        _ => String::new(),
    }
}

/// 提取工具输出文本。
///
/// 参数:
/// - `update`: 更新对象
///
/// 返回:
/// - 输出文本
fn tool_output_text(update: &Value) -> String {
    let text = content_text(update.get("content"));
    let structured = update
        .get("content")
        .filter(|content| contains_structured_tool_content(content))
        .map(value_to_arguments);
    if let Some(structured) = structured {
        if text.is_empty() {
            return structured;
        }
        return format!("{text}\n{structured}");
    }
    if let Some(raw_output) = update.get("rawOutput") {
        if text.is_empty() {
            return value_to_arguments(raw_output);
        }
        return format!("{text}\n{}", value_to_arguments(raw_output));
    }
    text
}

/// 判断工具内容是否包含 diff、terminal 或其它非纯文本结构。
///
/// 参数:
/// - `content`: ACP 工具 content 字段
///
/// 返回:
/// - 需要保留完整 JSON 时为 true
fn contains_structured_tool_content(content: &Value) -> bool {
    match content {
        Value::Array(items) => items.iter().any(contains_structured_tool_content),
        Value::Object(object) => {
            let kind = object
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if !kind.is_empty() && !matches!(kind, "content" | "text") {
                return true;
            }
            object
                .get("content")
                .is_some_and(contains_structured_tool_content)
        }
        _ => false,
    }
}

/// 把 ACP 附加元数据合并进工具参数 JSON。
///
/// 参数:
/// - `arguments`: 原始工具参数文本
/// - `key`: `_acp` 下的字段名
/// - `value`: 待保留的协议值
///
/// 返回:
/// - 合并后的 JSON 参数；原参数不是对象时使用包装对象
fn merge_acp_metadata(arguments: &str, key: &str, value: Value) -> String {
    let parsed = serde_json::from_str::<Value>(arguments)
        .unwrap_or_else(|_| serde_json::json!({ "value": arguments }));
    let mut object = match parsed {
        Value::Object(object) => object,
        value => serde_json::Map::from_iter([("value".to_string(), value)]),
    };
    let acp = object
        .entry("_acp".to_string())
        .or_insert_with(|| Value::Object(serde_json::Map::new()));
    if let Value::Object(acp) = acp {
        acp.insert(key.to_string(), value);
    }
    Value::Object(object).to_string()
}

/// 把任意 JSON 值转成工具参数文本。
///
/// 参数:
/// - `value`: 原始值
///
/// 返回:
/// - JSON 文本
fn value_to_arguments(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

/// 把 ACP 的计划条目转成 sai todo 工具的结果格式。
///
/// 复用既有的 todo 渲染：CLI 打印清单、TUI 落到沉底面板、Web 显示清单卡。
///
/// 参数:
/// - `entries`: plan 的条目数组
///
/// 返回:
/// - todo 工具结果 JSON
fn plan_to_todo_snapshot(entries: &[Value]) -> String {
    let items = entries
        .iter()
        .filter_map(|entry| {
            let text = entry.get("content").and_then(Value::as_str)?.trim();
            if text.is_empty() {
                return None;
            }
            let status = match entry
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("pending")
            {
                "in_progress" => "in_progress",
                "completed" => "completed",
                _ => "pending",
            };
            Some(serde_json::json!({ "text": text, "status": status }))
        })
        .collect::<Vec<_>>();
    serde_json::json!({ "ok": true, "items": items }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一条 session/update 参数。
    ///
    /// 参数:
    /// - `update`: update 对象
    ///
    /// 返回:
    /// - 通知参数
    fn params(update: Value) -> Value {
        serde_json::json!({ "sessionId": "s1", "update": update })
    }

    /// 使用独立桥接器翻译一条无需跨更新状态的通知。
    ///
    /// 参数:
    /// - `update`: update 对象
    ///
    /// 返回:
    /// - 翻译结果
    fn bridge(update: Value) -> BridgedUpdate {
        AcpEventBridge::default().bridge_session_update(&params(update))
    }

    #[test]
    fn maps_message_chunk_to_content() {
        let bridged = bridge(serde_json::json!({
            "sessionUpdate": "agent_message_chunk",
            "content": { "type": "text", "text": "你好" }
        }));
        assert_eq!(bridged.content_delta, "你好");
        assert!(matches!(
            bridged.events.first(),
            Some(AgentEvent::Chunk(chunk)) if chunk.kind == ChatStreamKind::Content
        ));
    }

    #[test]
    fn maps_thought_chunk_to_reasoning() {
        let bridged = bridge(serde_json::json!({
            "sessionUpdate": "agent_thought_chunk",
            "content": { "type": "text", "text": "先看配置" }
        }));
        assert_eq!(bridged.reasoning_delta, "先看配置");
        assert!(matches!(
            bridged.events.first(),
            Some(AgentEvent::Chunk(chunk)) if chunk.kind == ChatStreamKind::Reasoning
        ));
    }

    #[test]
    fn maps_tool_call_and_completion() {
        let mut bridge = AcpEventBridge::default();
        let call = bridge.bridge_session_update(&params(serde_json::json!({
            "sessionUpdate": "tool_call",
            "toolCallId": "t1",
            "title": "Read",
            "status": "pending",
            "rawInput": { "path": "a.rs" }
        })));
        match call.events.first() {
            Some(AgentEvent::ToolCallIdentified {
                id,
                name,
                arguments,
            }) => {
                assert_eq!(id, "t1");
                assert_eq!(name, "Read");
                assert!(arguments.contains("a.rs"));
            }
            other => panic!("expected a tool call, got {other:?}"),
        }

        let done = bridge.bridge_session_update(&params(serde_json::json!({
            "sessionUpdate": "tool_call_update",
            "toolCallId": "t1",
            "status": "completed",
            "content": [{ "type": "content", "content": { "type": "text", "text": "文件内容" } }]
        })));
        match done.events.first() {
            Some(AgentEvent::ToolResultIdentified {
                id,
                name,
                ok,
                output,
            }) => {
                assert_eq!(id, "t1");
                assert_eq!(name, "Read");
                assert!(ok);
                assert_eq!(output, "文件内容");
            }
            other => panic!("expected a tool result, got {other:?}"),
        }
    }

    #[test]
    fn maps_failed_tool_to_error_result() {
        let bridged = bridge(serde_json::json!({
            "sessionUpdate": "tool_call_update",
            "toolCallId": "t2",
            "title": "Shell",
            "status": "failed",
            "content": [{ "type": "content", "content": { "type": "text", "text": "exit 1" } }]
        }));
        assert!(matches!(
            bridged.events.first(),
            Some(AgentEvent::ToolResultIdentified { ok: false, .. })
        ));
    }

    /// Codex 的文件编辑可能在首条 tool_call 中直接完成，必须同时结束工具卡。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn maps_terminal_tool_call_to_started_and_completed_events() {
        let bridged = bridge(serde_json::json!({
            "sessionUpdate": "tool_call",
            "toolCallId": "edit-1",
            "title": "Editing files",
            "status": "completed",
            "content": [{ "type": "content", "content": { "type": "text", "text": "patch" } }]
        }));

        assert!(matches!(
            bridged.events.first(),
            Some(AgentEvent::ToolCallIdentified { id, .. }) if id == "edit-1"
        ));
        assert!(matches!(
            bridged.events.get(1),
            Some(AgentEvent::ToolResultIdentified { id, name, ok: true, output })
                if id == "edit-1" && name == "Editing files" && output == "patch"
        ));
    }

    /// plan 复用 sai 的 todo 渲染，因此要产出 todo 工具的结果结构。
    #[test]
    fn maps_plan_to_todo_snapshot() {
        let bridged = bridge(serde_json::json!({
            "sessionUpdate": "plan",
            "entries": [
                { "content": "读配置", "status": "completed" },
                { "content": "改解析", "status": "in_progress" },
                { "content": "补测试", "status": "pending" }
            ]
        }));
        match bridged.events.first() {
            Some(AgentEvent::ToolResult { name, output, .. }) => {
                assert_eq!(name, "todo");
                let value: Value = serde_json::from_str(output).unwrap();
                assert_eq!(value["items"].as_array().unwrap().len(), 3);
                assert_eq!(value["items"][1]["status"], "in_progress");
            }
            other => panic!("expected a todo result, got {other:?}"),
        }
    }

    /// 内核声明的斜杠命令要能被列出，用户才知道有什么可用。
    #[test]
    fn maps_available_commands_to_a_readable_list() {
        let bridged = bridge(serde_json::json!({
            "sessionUpdate": "available_commands_update",
            "availableCommands": [
                { "name": "review", "description": "review changes" },
                { "name": "compact", "description": "compact context" }
            ]
        }));
        match bridged.events.first() {
            Some(AgentEvent::ToolProgress { message, .. }) => {
                assert!(message.contains("/review"));
                assert!(message.contains("/compact"));
            }
            other => panic!("expected a command list, got {other:?}"),
        }
    }

    /// 空命令列表不产生噪音事件。
    #[test]
    fn ignores_empty_command_lists() {
        let bridged = bridge(serde_json::json!({
            "sessionUpdate": "available_commands_update",
            "availableCommands": []
        }));
        assert!(bridged.events.is_empty());
    }

    /// 标准 usage_update 应转换为 Sai 用量。
    #[test]
    fn maps_usage_update_to_context_usage() {
        let bridged = bridge(serde_json::json!({
            "sessionUpdate": "usage_update",
            "used": 1200,
            "size": 200000
        }));
        let usage = bridged.usage.expect("usage update");
        assert_eq!(usage.prompt_tokens, 1200);
        assert_eq!(usage.total_tokens, 1200);
    }

    /// 标准配置更新应保留完整 configOptions 状态。
    #[test]
    fn maps_config_option_update() {
        let bridged = bridge(serde_json::json!({
            "sessionUpdate": "config_option_update",
            "configOptions": [{
                "id": "model",
                "name": "Model",
                "category": "model",
                "type": "select",
                "currentValue": "claude-sonnet",
                "options": [{ "value": "claude-sonnet", "name": "Sonnet" }]
            }]
        }));
        assert_eq!(bridged.config_options.expect("config options").len(), 1);
    }

    /// 旧版模式更新也要进入运行状态，供客户端继续切换。
    #[test]
    fn maps_current_mode_update() {
        let bridged = bridge(serde_json::json!({
            "sessionUpdate": "current_mode_update",
            "currentModeId": "plan"
        }));
        assert_eq!(bridged.current_mode.as_deref(), Some("plan"));
    }

    /// diff、terminal 和位置必须随工具事件完整保留。
    #[test]
    fn preserves_structured_tool_content_and_locations() {
        let bridged = bridge(serde_json::json!({
            "sessionUpdate": "tool_call",
            "toolCallId": "edit-1",
            "title": "Edit",
            "rawInput": { "path": "/tmp/a.rs" },
            "locations": [{ "path": "/tmp/a.rs", "line": 3 }],
            "content": [{
                "type": "diff",
                "path": "/tmp/a.rs",
                "oldText": "old",
                "newText": "new"
            }]
        }));
        match bridged.events.first() {
            Some(AgentEvent::ToolCallIdentified { arguments, .. }) => {
                assert!(arguments.contains("locations"));
            }
            other => panic!("expected identified tool call, got {other:?}"),
        }
    }

    /// 图片、资源和终端引用不能在文本提取时丢失。
    #[test]
    fn preserves_non_text_tool_content() {
        let bridged = bridge(serde_json::json!({
            "sessionUpdate": "tool_call_update",
            "toolCallId": "inspect-1",
            "title": "Inspect",
            "status": "completed",
            "content": [{
                "type": "content",
                "content": { "type": "resource_link", "uri": "file:///tmp/a.rs", "name": "a.rs" }
            }]
        }));
        match bridged.events.first() {
            Some(AgentEvent::ToolResultIdentified { output, .. }) => {
                assert!(output.contains("resource_link"));
                assert!(output.contains("file:///tmp/a.rs"));
            }
            other => panic!("expected identified tool result, got {other:?}"),
        }
    }

    /// 协议仍在演进，未知更新类型不应中断对话。
    #[test]
    fn ignores_unknown_update_kinds() {
        let bridged = bridge(serde_json::json!({
            "sessionUpdate": "future_update",
            "value": 1
        }));
        assert!(bridged.events.is_empty());
    }
}
