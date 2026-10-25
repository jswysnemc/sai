use super::cell::HistoryCell;
use super::store::{TranscriptRenderOptions, TranscriptStore, TranscriptView};
use super::tool_cell::ToolCell;
use crate::i18n::text as t;
use crate::llm::ChatStreamKind;
use crate::render::{ReasoningDisplayMode, ToolCallDisplayMode};

const DETAIL_BYTE_LIMIT: usize = 8192;

/// 当前执行节点所需的纯文本快照，不包含终端控制序列或历史展开状态。
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ActivitySnapshot {
    pub(crate) title: String,
    pub(crate) details: String,
}

impl TranscriptStore {
    /// 【终端】【当前详情】提取当前步骤，参数流、思考和执行进度不会累计成面板列表。
    /// 参数: `options` 为可见性偏好；返回: 当前步骤的有界详情快照。
    pub(crate) fn activity_snapshot(&self, options: &TranscriptRenderOptions) -> ActivitySnapshot {
        // 1. 【终端】【当前详情】子会话只读取自己的概览，不泄露主会话执行详情
        if let TranscriptView::Subagent { label, .. } = &self.view {
            let detail = self
                .subagent_overview()
                .into_iter()
                .find(|entry| entry.viewing);
            return ActivitySnapshot {
                title: format!("{} · {label}", t("Subagent", "子智能体")),
                details: if options.tool_call_mode == ToolCallDisplayMode::Hidden {
                    String::new()
                } else {
                    bounded_text(
                        &detail.and_then(|entry| entry.detail).unwrap_or_default(),
                        true,
                    )
                },
            };
        }
        // 2. 【终端】【当前详情】新的参数和思考替换旧步骤，不继续显示已过时的工具详情
        if options.tool_call_mode != ToolCallDisplayMode::Hidden {
            if let Some(call) = &self.live_tool_call {
                return ActivitySnapshot {
                    title: format!("{} · {}", t("Preparing", "准备工具参数"), call.name),
                    details: arguments_text(&call.arguments_preview),
                };
            }
        }
        if let Some(tail) = &self.live_tail {
            if tail.kind == ChatStreamKind::Reasoning
                && options.reasoning_mode != ReasoningDisplayMode::Hidden
            {
                return ActivitySnapshot {
                    title: t("Thinking", "思考中").to_string(),
                    details: bounded_text(&tail.source, true),
                };
            }
            if tail.kind == ChatStreamKind::Content {
                return ActivitySnapshot {
                    title: t("Writing response", "生成回答").to_string(),
                    details: t(
                        "The response is displayed in the conversation above.",
                        "回答显示在上方会话区域。",
                    )
                    .to_string(),
                };
            }
        }
        // 3. 【终端】【当前详情】执行中的最新工具占用同一个槽位，保留输出末尾和最新进度
        if options.tool_call_mode != ToolCallDisplayMode::Hidden {
            if let Some(cell) = self
                .active_tool_index
                .and_then(|index| self.cells.get(index))
            {
                if let Some(snapshot) = cell_snapshot(cell) {
                    return snapshot;
                }
            }
        }
        if let Some(status) = self.work_status {
            let elapsed = self
                .work_status_started
                .map(|started| started.elapsed())
                .unwrap_or_default();
            return ActivitySnapshot {
                title: bounded_text(&status.render_line(0, elapsed), false),
                details: String::new(),
            };
        }
        ActivitySnapshot {
            title: t("Ready", "就绪").to_string(),
            details: t("Waiting for the next task.", "等待下一项任务。").to_string(),
        }
    }
}

/// 【终端】【工具详情】读取当前工具源数据，避免在固定区创建图片或重复执行 Markdown 渲染。
/// 参数: `cell` 为当前步骤；返回: 工具、编辑或子任务快照，其他类型返回空。
pub(super) fn cell_snapshot(cell: &HistoryCell) -> Option<ActivitySnapshot> {
    match cell {
        HistoryCell::Tool(ToolCell::Invocation(view)) => {
            let mut details = arguments_text(&view.arguments);
            for text in [
                view.progress.as_deref().unwrap_or(""),
                view.command_stdout_text().as_ref(),
                view.command_stderr_text().as_ref(),
            ] {
                if !text.is_empty() {
                    details.push('\n');
                    details.push_str(&bounded_text(text, true));
                }
            }
            Some(ActivitySnapshot {
                title: format!("{} · {}", t("Running", "执行中"), view.name),
                details: bounded_text(&details, true),
            })
        }
        HistoryCell::Diff(cell) => Some(ActivitySnapshot {
            title: t("Editing files", "编辑文件").to_string(),
            details: bounded_text(&cell.pager_body(), false),
        }),
        HistoryCell::Tool(ToolCell::Subagent(cell)) => {
            let overview = cell.overview();
            Some(ActivitySnapshot {
                title: format!("{} · {}", t("Subagent", "子智能体"), overview.label),
                details: bounded_text(&overview.detail.unwrap_or_default(), true),
            })
        }
        HistoryCell::Tool(ToolCell::CompactionStarted { turn_count, model }) => {
            Some(ActivitySnapshot {
                title: t("Compacting context", "压缩上下文").to_string(),
                details: bounded_text(&format!("{model}\n{turn_count} turns"), false),
            })
        }
        _ => None,
    }
}

/// 【终端】【参数详情】格式化有界 JSON 参数，未完成的参数流保留原文。
/// 参数: `arguments` 为工具参数；返回: 有界纯文本。
fn arguments_text(arguments: &str) -> String {
    let text = bounded_text(arguments, true);
    let formatted = serde_json::from_str::<serde_json::Value>(&text)
        .ok()
        .and_then(|value| serde_json::to_string_pretty(&value).ok())
        .unwrap_or(text);
    bounded_text(&formatted, true)
}

/// 【终端】【详情清理】限制读取量并去除控制序列，输出末尾优先展示最新进度。
/// 参数: `text` 为原文，`tail` 表示保留末尾；返回: 安全的有界纯文本。
fn bounded_text(text: &str, tail: bool) -> String {
    let bounded = if text.len() <= DETAIL_BYTE_LIMIT {
        text
    } else if tail {
        let mut start = text.len() - DETAIL_BYTE_LIMIT;
        while !text.is_char_boundary(start) {
            start += 1;
        }
        &text[start..]
    } else {
        let mut end = DETAIL_BYTE_LIMIT;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        &text[..end]
    };
    let mut cleaned: String = crate::render::activity_animation::strip_ansi_for_test(bounded)
        .replace('\r', "\n")
        .replace('\t', "    ")
        .chars()
        .filter(|character| !character.is_control() || *character == '\n')
        .collect();
    if cleaned.len() > DETAIL_BYTE_LIMIT {
        if tail {
            let mut start = cleaned.len() - DETAIL_BYTE_LIMIT;
            while !cleaned.is_char_boundary(start) {
                start += 1;
            }
            cleaned.drain(..start);
        } else {
            let mut end = DETAIL_BYTE_LIMIT;
            while !cleaned.is_char_boundary(end) {
                end -= 1;
            }
            cleaned.truncate(end);
        }
    }
    cleaned
}

#[cfg(test)]
#[path = "activity_snapshot_tests.rs"]
mod tests;
