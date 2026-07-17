use super::subagent_state;
use serde_json::Value;

/// 子代理推理片段消息前缀。
const REASONING_PREFIX: &str = "__subagent_reasoning__";
/// 子代理轮间正文消息前缀。
const TEXT_PREFIX: &str = "__subagent_text__";
/// 子工具调用开始消息前缀。
const TOOL_CALL_PREFIX: &str = "__subtool_call__";
/// 子工具调用结果消息前缀。
const TOOL_RESULT_PREFIX: &str = "__subtool_result__";

/// 消费子代理进度消息,写入时间线并同步快照。
///
/// 参数:
/// - `subagent_id`: 子智能体 ID
/// - `progress_rx`: 进度消息接收器
///
/// 返回:
/// - 无
pub(crate) async fn consume_progress(
    subagent_id: String,
    mut progress_rx: tokio::sync::mpsc::UnboundedReceiver<String>,
) {
    while let Some(message) = progress_rx.recv().await {
        dispatch_message(&subagent_id, &message);
    }
}

/// 按前缀分发一条进度消息。
///
/// 参数:
/// - `subagent_id`: 子智能体 ID
/// - `message`: 子代理上报的原始消息
///
/// 返回:
/// - 无
fn dispatch_message(subagent_id: &str, message: &str) {
    // 1. 结构化消息:工具调用开始/结束写入时间线并同步 step/phase
    if let Some(payload) = message.strip_prefix(TOOL_CALL_PREFIX) {
        if let Some((name, args)) = parse_tool_call(payload) {
            subagent_state::timeline_tool_started(subagent_id, &name, &args);
        }
        return;
    }
    if let Some(payload) = message.strip_prefix(TOOL_RESULT_PREFIX) {
        if let Some((name, ok, output)) = parse_tool_result(payload) {
            subagent_state::timeline_tool_finished(subagent_id, &name, ok, &output);
        }
        return;
    }
    // 2. 流式文本:推理与轮间正文聚合进时间线
    if let Some(text) = message.strip_prefix(REASONING_PREFIX) {
        subagent_state::timeline_streaming_text(subagent_id, text, true);
        return;
    }
    if let Some(text) = message.strip_prefix(TEXT_PREFIX) {
        subagent_state::timeline_streaming_text(subagent_id, text, false);
        return;
    }
    // 3. 普通文本:作为阶段说明写回快照(如最终统计行)
    subagent_state::update_subagent_progress(subagent_id, parse_progress_message(message));
}

/// 解析子工具调用消息载荷。
///
/// 参数:
/// - `payload`: JSON 文本 {name, args}
///
/// 返回:
/// - 工具名称与参数文本
fn parse_tool_call(payload: &str) -> Option<(String, String)> {
    let value = serde_json::from_str::<Value>(payload).ok()?;
    let name = value.get("name")?.as_str()?.to_string();
    let args = value
        .get("args")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    Some((name, args))
}

/// 解析子工具结果消息载荷。
///
/// 参数:
/// - `payload`: JSON 文本 {name, ok, output}
///
/// 返回:
/// - 工具名称、成功标记与输出文本
fn parse_tool_result(payload: &str) -> Option<(String, bool, String)> {
    let value = serde_json::from_str::<Value>(payload).ok()?;
    let name = value.get("name")?.as_str()?.to_string();
    let ok = value.get("ok").and_then(Value::as_bool).unwrap_or(false);
    let output = value
        .get("output")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    Some((name, ok, output))
}

/// 从进度文本解析出结构化进度更新。
///
/// 参数:
/// - `message`: 子代理上报的进度文本
///
/// 返回:
/// - 结构化进度更新
fn parse_progress_message(message: &str) -> subagent_state::SubagentProgressUpdate {
    let mut update = subagent_state::SubagentProgressUpdate {
        phase: Some(message.to_string()),
        ..Default::default()
    };
    // 1. 识别 "工具 #N：名称 ..." 或 "tool #N: name ..." 形式，提取步数与工具名
    let marker = message.find('#');
    if let Some(marker) = marker {
        let rest = &message[marker + 1..];
        let digits = rest
            .chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>();
        if let Ok(step) = digits.parse::<usize>() {
            update.step = Some(step);
            let after = rest[digits.len()..]
                .trim_start_matches([':', '：', ' '])
                .trim();
            let name = after
                .split([' ', '：', ':'])
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            if !name.is_empty() {
                update.last_tool = Some(name);
            }
        }
    }
    update
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::subagent_state::{create_subagent, subagent_snapshot, subagent_timeline};
    use crate::tools::subagent_timeline::SubagentTimelineEntry;
    use serde_json::json;

    #[test]
    fn parses_chinese_tool_progress() {
        let update = parse_progress_message("工具 #3：Shell 运行中");

        assert_eq!(update.step, Some(3));
        assert_eq!(update.last_tool.as_deref(), Some("Shell"));
        assert_eq!(update.phase.as_deref(), Some("工具 #3：Shell 运行中"));
    }

    #[test]
    fn parses_english_tool_progress() {
        let update = parse_progress_message("tool #12: read_file running");

        assert_eq!(update.step, Some(12));
        assert_eq!(update.last_tool.as_deref(), Some("read_file"));
    }

    #[test]
    fn keeps_plain_phase_without_step() {
        let update = parse_progress_message("工具调用 5 次　消耗 Token 1.2K");

        assert_eq!(update.step, None);
        assert_eq!(update.last_tool, None);
        assert_eq!(
            update.phase.as_deref(),
            Some("工具调用 5 次　消耗 Token 1.2K")
        );
    }

    #[test]
    fn structured_messages_feed_timeline_and_snapshot() {
        let (subagent, _cancel) = create_subagent("feed".to_string(), "explore".to_string(), 5);

        dispatch_message(
            &subagent.id,
            &format!(
                "{TOOL_CALL_PREFIX}{}",
                json!({"name": "read_file", "args": "{\"path\":\"a.rs\"}"})
            ),
        );
        dispatch_message(&subagent.id, &format!("{REASONING_PREFIX}分析中"));
        dispatch_message(
            &subagent.id,
            &format!(
                "{TOOL_RESULT_PREFIX}{}",
                json!({"name": "read_file", "ok": true, "output": "content"})
            ),
        );
        dispatch_message(&subagent.id, &format!("{TEXT_PREFIX}第一轮结论"));

        let snapshot = subagent_snapshot(&subagent.id).unwrap();
        assert_eq!(snapshot.step, 1);
        assert_eq!(snapshot.last_tool.as_deref(), Some("read_file"));
        let timeline = subagent_timeline(&subagent.id).unwrap();
        assert_eq!(timeline.len(), 3);
        assert!(matches!(
            &timeline[0],
            SubagentTimelineEntry::Tool { ok: Some(true), .. }
        ));
        assert!(matches!(
            &timeline[1],
            SubagentTimelineEntry::Reasoning { text } if text == "分析中"
        ));
        assert!(matches!(
            &timeline[2],
            SubagentTimelineEntry::Text { text } if text == "第一轮结论"
        ));
    }
}
