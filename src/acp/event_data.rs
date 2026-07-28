use serde_json::Value;

/// 提取内核声明的可用命令名。
///
/// 参数:
/// - `update`: `available_commands_update` 对象
///
/// 返回:
/// - 以空格分隔的 `/命令` 列表；没有命令时返回 None
pub(super) fn command_names(update: &Value) -> Option<String> {
    let commands = update.get("availableCommands")?.as_array()?;
    let names = commands
        .iter()
        .filter_map(|command| command.get("name").and_then(Value::as_str))
        .map(|name| format!("/{name}"))
        .collect::<Vec<_>>();
    (!names.is_empty()).then(|| names.join(" "))
}

/// 把 ACP 的计划条目转成 Sai todo 工具结果。
///
/// 参数:
/// - `entries`: plan 条目数组
///
/// 返回:
/// - todo 工具结果 JSON
pub(super) fn plan_to_todo_snapshot(entries: &[Value]) -> String {
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
