use super::ToolView;
use crate::render::{
    status_style::ToolHealth, tool_event_line::tool_status_line, ToolCallDisplayMode,
};

/// 【终端】【通知识别】识别持久化的完成回执或网格通知，普通用户正文不据此转换。
/// 参数: `text` 为自动轮次正文；返回: 是否含有外部事件的完整包装。
pub(crate) fn is_external_notice(text: &str) -> bool {
    external_notice_body(text).is_some()
}

/// 【终端】【通知正文】提取完整事件包装，兼容前面附有目标上下文的自动轮次。
/// 参数: `text` 为持久化自动输入；返回: 事件正文，没有完整包装时为空。
fn external_notice_body(text: &str) -> Option<&str> {
    [
        (
            "<external-completion-events>",
            "</external-completion-events>",
        ),
        ("<mesh-message>", "</mesh-message>"),
    ]
    .iter()
    .find_map(|(open, close)| {
        let (_, body) = text.split_once(open)?;
        let (body, _) = body.split_once(close)?;
        Some(body)
    })
}

/// 【终端】【后台通知】把自动完成通知按工具卡展示，不再使用用户消息回显。
/// 参数: `view` 为通知单元，`mode` 为展示模式；返回: 通知视图，其他工具为空。
pub(super) fn render(view: &ToolView, mode: ToolCallDisplayMode) -> Option<String> {
    if view.name != "background_notice" {
        return None;
    }
    if mode == ToolCallDisplayMode::Hidden {
        return Some(String::new());
    }
    let source = view
        .outcome
        .as_ref()
        .map(|outcome| outcome.output.as_str())
        .unwrap_or_default();
    let source = source.trim();
    let source = external_notice_body(source).unwrap_or(source).trim();
    let source = if source.starts_with("以下后台工作") || source.starts_with("以下网格通信")
    {
        source
            .split_once("\n\n")
            .map(|(_, details)| details)
            .unwrap_or_default()
    } else {
        source
    };
    let mut details = Vec::new();
    let mut status = String::new();
    let mut kind = "work";
    for line in source.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with("后台工作已完成")
            || line.starts_with("后台任务已完成")
            || line.starts_with("Background work completed")
        {
            continue;
        }
        if let Some(value) = field(line, &["状态:", "状态：", "Status:"]) {
            status = value
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_string();
            details.push(format!("Status: {value}"));
        } else if let Some(value) = field(
            line,
            &["后台命令:", "后台命令：", "Background command:", "Command:"],
        ) {
            kind = "command";
            details.push(format!("Command: {value}"));
        } else if let Some(value) = field(line, &["子 Agent:", "子 Agent：", "Subagent:"]) {
            kind = "subagent";
            details.push(format!("Subagent: {value}"));
        } else if line.starts_with("网格消息") || line.starts_with("Mesh message") {
            kind = "mesh";
        } else if let Some(value) = field(line, &["来自:", "来自：", "From:"]) {
            kind = "mesh";
            details.push(format!("From: {value}"));
        } else if let Some(value) = field(line, &["目标:", "目标：", "To:"]) {
            details.push(format!("To: {value}"));
        } else if let Some(value) = field(line, &["correlation_id:", "correlation_id："]) {
            details.push(format!("Correlation ID: {value}"));
        } else if let Some(value) = field(line, &["正文:", "正文：", "Message:"]) {
            details.push(format!("Message: {value}"));
        } else if let Some(value) = field(line, &["说明:", "说明：", "Note:"]) {
            let note = if value.starts_with("日志未附带") {
                "Logs are not attached. Use background_command action=output to read logs; head_lines or tail_lines selects the range."
            } else if value.starts_with("结果未附带") {
                "Results are not attached. Use subagent action=result to read results."
            } else if value.starts_with("持久子代理已完成") {
                "The subagent finished its current task and is idle. Use subagent action=result to read results, action=send to add instructions, or action=stop to finish."
            } else {
                value
            };
            details.push(format!("Note: {note}"));
        } else {
            details.push(line.to_string());
        }
    }
    let target = match kind {
        "command" => "Background command",
        "subagent" => "Subagent",
        "mesh" => "Mesh message",
        _ => "Background work",
    };
    let (verb, health) = match status.as_str() {
        "stopped" | "cancelled" => ("stopped", ToolHealth::Neutral),
        "failed" => ("failed", ToolHealth::Err),
        "timed_out" => ("timed out", ToolHealth::Err),
        "idle" => ("idle", ToolHealth::Ok),
        _ if kind == "mesh" => ("received", ToolHealth::Neutral),
        _ => ("completed", ToolHealth::Ok),
    };
    let mut label = format!("{target} {verb}");
    if mode == ToolCallDisplayMode::Summary {
        if let Some(subject) = details
            .iter()
            .find_map(|line| field(line, &["Command:", "Subagent:", "From:"]))
        {
            label.push_str(&format!(" · {subject}"));
        }
        return Some(tool_status_line(&label, "", health));
    }
    let mut output = tool_status_line(&label, "", health);
    if details.is_empty() {
        details.push("Continuing the conversation automatically".into());
    }
    for (position, line) in details.iter().enumerate() {
        output.push_str(&format!(
            "\n\x1b[2m{}{line}\x1b[0m",
            if position == 0 { "  └ " } else { "    " }
        ));
    }
    Some(output)
}

/// 【终端】【通知字段】识别内部通知的中英文标签，正文内容保持不变。
/// 参数: `line` 为通知行，`prefixes` 为标签集合；返回: 去掉标签后的字段正文。
fn field<'a>(line: &'a str, prefixes: &[&str]) -> Option<&'a str> {
    prefixes
        .iter()
        .find_map(|prefix| line.strip_prefix(prefix).map(str::trim))
}
