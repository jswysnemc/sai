use serde::{Deserialize, Serialize};

/// 单条时间线条目的文本长度上限。
const TEXT_ENTRY_LIMIT: usize = 4000;
/// 工具参数预览长度上限。
const ARGS_PREVIEW_LIMIT: usize = 240;
/// 工具输出预览长度上限。
const OUTPUT_PREVIEW_LIMIT: usize = 600;
/// 时间线条目总数上限,超过后滚动丢弃最早的条目。
const MAX_ENTRIES: usize = 240;

/// 子智能体执行时间线中的一个条目。
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum SubagentTimelineEntry {
    /// 一次子工具调用,ok 为空表示仍在运行。
    Tool {
        step: usize,
        name: String,
        args_preview: String,
        ok: Option<bool>,
        output_preview: Option<String>,
    },
    /// 子智能体轮间输出的正文文本。
    Text { text: String },
    /// 子智能体的推理片段。
    Reasoning { text: String },
}

/// 子智能体执行时间线,按发生顺序聚合工具调用、正文与推理。
#[derive(Debug, Clone, Default)]
pub(crate) struct SubagentTimeline {
    entries: Vec<SubagentTimelineEntry>,
    tool_count: usize,
}

impl SubagentTimeline {
    /// 从持久化条目恢复时间线。
    ///
    /// 参数:
    /// - `entries`: 已保存的时间线条目
    ///
    /// 返回:
    /// - 恢复后的时间线
    pub(crate) fn from_entries(entries: Vec<SubagentTimelineEntry>) -> Self {
        let tool_count = entries
            .iter()
            .filter_map(|entry| match entry {
                SubagentTimelineEntry::Tool { step, .. } => Some(*step),
                SubagentTimelineEntry::Text { .. } | SubagentTimelineEntry::Reasoning { .. } => {
                    None
                }
            })
            .max()
            .unwrap_or_default();
        Self {
            entries,
            tool_count,
        }
    }

    /// 记录一次子工具调用开始。
    ///
    /// 参数:
    /// - `name`: 子工具名称
    /// - `args`: 子工具参数 JSON 文本
    ///
    /// 返回:
    /// - 该调用的 1 起始步数
    pub(crate) fn push_tool(&mut self, name: &str, args: &str) -> usize {
        self.tool_count += 1;
        self.push_entry(SubagentTimelineEntry::Tool {
            step: self.tool_count,
            name: name.to_string(),
            args_preview: truncate_chars(args, ARGS_PREVIEW_LIMIT),
            ok: None,
            output_preview: None,
        });
        self.tool_count
    }

    /// 回填最近一次同名未完成调用的结果。
    ///
    /// 参数:
    /// - `name`: 子工具名称
    /// - `ok`: 是否成功
    /// - `output`: 子工具输出
    ///
    /// 返回:
    /// - 命中的调用步数,未命中时为空
    pub(crate) fn complete_tool(&mut self, name: &str, ok: bool, output: &str) -> Option<usize> {
        let entry = self.entries.iter_mut().rev().find(|entry| {
            matches!(entry, SubagentTimelineEntry::Tool { name: entry_name, ok: None, .. } if entry_name == name)
        })?;
        let SubagentTimelineEntry::Tool {
            step,
            ok: entry_ok,
            output_preview,
            ..
        } = entry
        else {
            return None;
        };
        *entry_ok = Some(ok);
        *output_preview = Some(truncate_chars(output, OUTPUT_PREVIEW_LIMIT));
        Some(*step)
    }

    /// 追加子智能体正文文本,与最近的正文条目聚合。
    ///
    /// 参数:
    /// - `text`: 正文片段
    ///
    /// 返回:
    /// - 无
    pub(crate) fn append_text(&mut self, text: &str) {
        self.append_streaming(text, false);
    }

    /// 追加子智能体推理片段,与最近的推理条目聚合。
    ///
    /// 参数:
    /// - `text`: 推理片段
    ///
    /// 返回:
    /// - 无
    pub(crate) fn append_reasoning(&mut self, text: &str) {
        self.append_streaming(text, true);
    }

    /// 导出全部时间线条目。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 条目快照列表
    pub(crate) fn entries(&self) -> Vec<SubagentTimelineEntry> {
        self.entries.clone()
    }

    /// 聚合流式文本:末尾同类条目未满时追加,否则新开条目。
    fn append_streaming(&mut self, text: &str, reasoning: bool) {
        if text.is_empty() {
            return;
        }
        if let Some(last) = self.entries.last_mut() {
            let target = match (reasoning, last) {
                (true, SubagentTimelineEntry::Reasoning { text }) => Some(text),
                (false, SubagentTimelineEntry::Text { text }) => Some(text),
                _ => None,
            };
            if let Some(existing) = target {
                if existing.chars().count() < TEXT_ENTRY_LIMIT {
                    existing.push_str(text);
                    return;
                }
            }
        }
        let entry = if reasoning {
            SubagentTimelineEntry::Reasoning {
                text: text.to_string(),
            }
        } else {
            SubagentTimelineEntry::Text {
                text: text.to_string(),
            }
        };
        self.push_entry(entry);
    }

    /// 写入条目并滚动丢弃超限的最早条目。
    fn push_entry(&mut self, entry: SubagentTimelineEntry) {
        self.entries.push(entry);
        if self.entries.len() > MAX_ENTRIES {
            self.entries.remove(0);
        }
    }
}

/// 按字符数截断文本。
///
/// 参数:
/// - `text`: 原始文本
/// - `limit`: 字符数上限
///
/// 返回:
/// - 截断后的文本,超限时附省略号
fn truncate_chars(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    let mut result = text.chars().take(limit).collect::<String>();
    result.push('…');
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_calls_are_numbered_and_completed_in_order() {
        let mut timeline = SubagentTimeline::default();
        assert_eq!(timeline.push_tool("read_file", "{}"), 1);
        assert_eq!(timeline.push_tool("grep", "{}"), 2);

        assert_eq!(timeline.complete_tool("grep", true, "matched"), Some(2));
        assert_eq!(timeline.complete_tool("read_file", false, "err"), Some(1));
        assert_eq!(timeline.complete_tool("missing", true, ""), None);

        let entries = timeline.entries();
        assert_eq!(entries.len(), 2);
        let SubagentTimelineEntry::Tool {
            ok, output_preview, ..
        } = &entries[0]
        else {
            panic!("expected tool entry");
        };
        assert_eq!(*ok, Some(false));
        assert_eq!(output_preview.as_deref(), Some("err"));
    }

    #[test]
    fn streaming_text_merges_into_last_entry_of_same_kind() {
        let mut timeline = SubagentTimeline::default();
        timeline.append_reasoning("先看");
        timeline.append_reasoning("目录");
        timeline.append_text("# 结论\n");
        timeline.append_text("一切正常");
        timeline.append_reasoning("再想想");

        let entries = timeline.entries();
        assert_eq!(entries.len(), 3);
        assert!(
            matches!(&entries[0], SubagentTimelineEntry::Reasoning { text } if text == "先看目录")
        );
        assert!(
            matches!(&entries[1], SubagentTimelineEntry::Text { text } if text == "# 结论\n一切正常")
        );
    }

    #[test]
    fn entries_roll_over_when_exceeding_capacity() {
        let mut timeline = SubagentTimeline::default();
        for index in 0..(MAX_ENTRIES + 5) {
            timeline.push_tool("t", &format!("{index}"));
            // 打断聚合,强制每次新开条目
            timeline.complete_tool("t", true, "");
        }
        assert_eq!(timeline.entries().len(), MAX_ENTRIES);
    }

    #[test]
    fn previews_are_truncated() {
        let mut timeline = SubagentTimeline::default();
        timeline.push_tool("t", &"a".repeat(500));
        timeline.complete_tool("t", true, &"b".repeat(2000));

        let entries = timeline.entries();
        let SubagentTimelineEntry::Tool {
            args_preview,
            output_preview,
            ..
        } = &entries[0]
        else {
            panic!("expected tool entry");
        };
        assert_eq!(args_preview.chars().count(), 241);
        assert_eq!(output_preview.as_ref().unwrap().chars().count(), 601);
    }
}
