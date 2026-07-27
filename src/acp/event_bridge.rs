use crate::agent::AgentEvent;
use crate::llm::{ChatStreamChunk, ChatStreamKind};
use serde_json::Value;

/// 一条 `session/update` 翻译出的结果。
#[derive(Debug, Default)]
pub(crate) struct BridgedUpdate {
    /// 需要转发给 UI 的事件
    pub(crate) events: Vec<AgentEvent>,
    /// 助手正文增量，用于累计本轮回复
    pub(crate) content_delta: String,
    /// 思考增量，用于累计本轮推理
    pub(crate) reasoning_delta: String,
}

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
pub(crate) fn bridge_session_update(params: &Value) -> BridgedUpdate {
    let mut bridged = BridgedUpdate::default();
    let Some(update) = params.get("update") else {
        return bridged;
    };
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
        "tool_call" => {
            let name = tool_name(update);
            let arguments = update
                .get("rawInput")
                .map(value_to_arguments)
                .unwrap_or_else(|| "{}".to_string());
            bridged.events.push(AgentEvent::ToolCall { name, arguments });
        }
        "tool_call_update" => bridge_tool_update(update, &mut bridged),
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
        _ => {}
    }
    bridged
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

/// 翻译工具状态更新。
///
/// 进行中映射为进度事件，终态映射为结果事件——这正是 sai 工具卡的两个阶段。
///
/// 参数:
/// - `update`: 更新对象
/// - `bridged`: 累积结果
///
/// 返回:
/// - 无
fn bridge_tool_update(update: &Value, bridged: &mut BridgedUpdate) {
    let name = tool_name(update);
    let status = update
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let output = tool_output_text(update);
    match status {
        "completed" => bridged.events.push(AgentEvent::ToolResult {
            name,
            ok: true,
            output,
        }),
        "failed" => bridged.events.push(AgentEvent::ToolResult {
            name,
            ok: false,
            output,
        }),
        // pending / in_progress 都还没有结论，作为进度展示
        _ => {
            if !output.is_empty() {
                bridged.events.push(AgentEvent::ToolProgress {
                    name,
                    message: output,
                });
            }
        }
    }
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
    if !text.is_empty() {
        return text;
    }
    update
        .get("rawOutput")
        .map(value_to_arguments)
        .unwrap_or_default()
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
            let status = match entry.get("status").and_then(Value::as_str).unwrap_or("pending") {
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

    #[test]
    fn maps_message_chunk_to_content() {
        let bridged = bridge_session_update(&params(serde_json::json!({
            "sessionUpdate": "agent_message_chunk",
            "content": { "type": "text", "text": "你好" }
        })));
        assert_eq!(bridged.content_delta, "你好");
        assert!(matches!(
            bridged.events.first(),
            Some(AgentEvent::Chunk(chunk)) if chunk.kind == ChatStreamKind::Content
        ));
    }

    #[test]
    fn maps_thought_chunk_to_reasoning() {
        let bridged = bridge_session_update(&params(serde_json::json!({
            "sessionUpdate": "agent_thought_chunk",
            "content": { "type": "text", "text": "先看配置" }
        })));
        assert_eq!(bridged.reasoning_delta, "先看配置");
        assert!(matches!(
            bridged.events.first(),
            Some(AgentEvent::Chunk(chunk)) if chunk.kind == ChatStreamKind::Reasoning
        ));
    }

    #[test]
    fn maps_tool_call_and_completion() {
        let call = bridge_session_update(&params(serde_json::json!({
            "sessionUpdate": "tool_call",
            "toolCallId": "t1",
            "title": "Read",
            "status": "pending",
            "rawInput": { "path": "a.rs" }
        })));
        match call.events.first() {
            Some(AgentEvent::ToolCall { name, arguments }) => {
                assert_eq!(name, "Read");
                assert!(arguments.contains("a.rs"));
            }
            other => panic!("expected a tool call, got {other:?}"),
        }

        let done = bridge_session_update(&params(serde_json::json!({
            "sessionUpdate": "tool_call_update",
            "toolCallId": "t1",
            "title": "Read",
            "status": "completed",
            "content": [{ "type": "content", "content": { "type": "text", "text": "文件内容" } }]
        })));
        match done.events.first() {
            Some(AgentEvent::ToolResult { name, ok, output }) => {
                assert_eq!(name, "Read");
                assert!(ok);
                assert_eq!(output, "文件内容");
            }
            other => panic!("expected a tool result, got {other:?}"),
        }
    }

    #[test]
    fn maps_failed_tool_to_error_result() {
        let bridged = bridge_session_update(&params(serde_json::json!({
            "sessionUpdate": "tool_call_update",
            "title": "Shell",
            "status": "failed",
            "content": [{ "type": "content", "content": { "type": "text", "text": "exit 1" } }]
        })));
        assert!(matches!(
            bridged.events.first(),
            Some(AgentEvent::ToolResult { ok: false, .. })
        ));
    }

    /// plan 复用 sai 的 todo 渲染，因此要产出 todo 工具的结果结构。
    #[test]
    fn maps_plan_to_todo_snapshot() {
        let bridged = bridge_session_update(&params(serde_json::json!({
            "sessionUpdate": "plan",
            "entries": [
                { "content": "读配置", "status": "completed" },
                { "content": "改解析", "status": "in_progress" },
                { "content": "补测试", "status": "pending" }
            ]
        })));
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
        let bridged = bridge_session_update(&params(serde_json::json!({
            "sessionUpdate": "available_commands_update",
            "availableCommands": [
                { "name": "review", "description": "review changes" },
                { "name": "compact", "description": "compact context" }
            ]
        })));
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
        let bridged = bridge_session_update(&params(serde_json::json!({
            "sessionUpdate": "available_commands_update",
            "availableCommands": []
        })));
        assert!(bridged.events.is_empty());
    }

    /// 协议仍在演进，未知更新类型不应中断对话。
    #[test]
    fn ignores_unknown_update_kinds() {
        let bridged = bridge_session_update(&params(serde_json::json!({
            "sessionUpdate": "usage_update",
            "usage": { "inputTokens": 10 }
        })));
        assert!(bridged.events.is_empty());
    }
}
