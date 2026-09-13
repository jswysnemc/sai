use crate::agent::AgentEvent;
use crate::llm::ToolCallStreamProgress;
use crate::web::runs::WebEvent;
use serde_json::Value;

/// 【终端】【远端工具回放】将共享事件还原为本地工具生命周期事件。
/// 参数: `event` 为持有者广播的工具事件
/// 返回: 可交给统一渲染链路的事件；缺少必要字段时为空
pub(super) fn decode_tool_event(event: &WebEvent) -> Option<AgentEvent> {
    let payload = &event.payload;
    let name = payload.get("name").and_then(Value::as_str);
    if event.kind == "tool.call.preparing" {
        let preview = text_field(payload, "arguments_preview");
        return Some(AgentEvent::ToolCallProgress(ToolCallStreamProgress {
            index: usize_field(payload, "index").unwrap_or(0),
            name: name.map(str::to_string),
            arguments_chars: usize_field(payload, "arguments_chars")
                .unwrap_or_else(|| preview.chars().count()),
            arguments_bytes: usize_field(payload, "arguments_bytes").unwrap_or(preview.len()),
            arguments_preview: preview,
        }));
    }
    let name = name?.to_string();
    let id = payload.get("tool_id").and_then(Value::as_str);
    match event.kind.as_str() {
        "tool.call.started" => {
            let arguments = text_field(payload, "arguments");
            Some(match id {
                Some(id) => AgentEvent::ToolCallIdentified {
                    id: id.into(),
                    name,
                    arguments,
                },
                None => AgentEvent::ToolCall { name, arguments },
            })
        }
        "tool.progress" => {
            let message = text_field(payload, "message");
            Some(match id {
                Some(id) => AgentEvent::ToolProgressIdentified {
                    id: id.into(),
                    name,
                    message,
                },
                None => AgentEvent::ToolProgress { name, message },
            })
        }
        "tool.result" => {
            let ok = payload.get("ok").and_then(Value::as_bool)?;
            let output = text_field(payload, "output");
            Some(match id {
                Some(id) => AgentEvent::ToolResultIdentified {
                    id: id.into(),
                    name,
                    ok,
                    output,
                },
                None => AgentEvent::ToolResult { name, ok, output },
            })
        }
        _ => None,
    }
}

/// 【终端】【远端工具回放】兼容字符串和结构化 JSON 载荷。
/// 参数: `payload` 为事件数据，`key` 为字段名
/// 返回: 原始文本；字段缺失时为空
fn text_field(payload: &Value, key: &str) -> String {
    match payload.get(key) {
        Some(Value::String(text)) => text.clone(),
        None | Some(Value::Null) => String::new(),
        Some(value) => value.to_string(),
    }
}

/// 【终端】【远端工具回放】读取适配当前平台的非负计数。
/// 参数: `payload` 为事件数据，`key` 为字段名
/// 返回: 有效计数，类型不符或溢出时为空
fn usize_field(payload: &Value, key: &str) -> Option<usize> {
    payload.get(key)?.as_u64()?.try_into().ok()
}
