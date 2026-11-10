use super::*;

/// 【子任务】【响应恢复】撤销当前工具边界后的失败响应，保留之前完成的任务记录
/// 参数：子代理标识、正文和推理字符数；返回无
pub(crate) fn rewind_response(id: &str, mut content: usize, mut reasoning: usize) {
    let mut records = subagents().lock().expect("subagent state lock");
    let Some(record) = records.get_mut(id) else {
        return;
    };
    let mut entries = record.timeline.entries();
    let mut index = entries.len();
    while index > 0 && content + reasoning > 0 {
        index -= 1;
        let empty = match &mut entries[index] {
            SubagentTimelineEntry::Text { text } if content > 0 => {
                content = crate::llm::remove_text_suffix(text, content);
                text.is_empty()
            }
            SubagentTimelineEntry::Reasoning { text } if reasoning > 0 => {
                reasoning = crate::llm::remove_text_suffix(text, reasoning);
                text.is_empty()
            }
            SubagentTimelineEntry::Tool { .. } | SubagentTimelineEntry::Message { .. } => break,
            _ => false,
        };
        if empty {
            entries.remove(index);
        }
    }
    record.timeline = SubagentTimeline::from_entries(entries);
    publish_record(record);
    let owner = record.owner_key.clone();
    persist_owner_locked(&records, &owner);
}
