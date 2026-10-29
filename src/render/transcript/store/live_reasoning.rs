use super::TranscriptStore;
#[cfg(test)]
use crate::llm::ChatStreamKind;
#[cfg(test)]
use crate::render::transcript::store::HistoryCell;
#[cfg(test)]
use crate::render::transcript::tool_cell::ToolCell;

impl TranscriptStore {
    /// 切换当前实时思考块的展开状态。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 当前没有可切换思考块时返回 false
    #[cfg(test)]
    pub(crate) fn toggle_live_reasoning(&mut self) -> bool {
        let Some(tail) = self
            .live_tail
            .as_mut()
            .filter(|tail| tail.kind == ChatStreamKind::Reasoning && !tail.source.is_empty())
        else {
            return false;
        };
        tail.expanded = !tail.expanded;
        true
    }

    /// 切换最近一块可折叠内容：live 思考、diff、定稿思考、命令输出或粘贴回显。
    ///
    /// Ctrl+O 只切「刚看到的那块」，并失效渲染缓存，否则定稿 diff 展开后仍画旧行。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 找到可切换块时返回 true
    #[cfg(test)]
    pub(crate) fn toggle_inline_expand(&mut self) -> bool {
        if self.toggle_live_reasoning() {
            return true;
        }
        for index in (0..self.cells.len()).rev() {
            let toggled = match self.cells.get_mut(index) {
                Some(HistoryCell::Diff(cell)) => {
                    cell.toggle_expanded();
                    true
                }
                Some(HistoryCell::Reasoning(cell)) if !cell.source.trim().is_empty() => {
                    cell.toggle_expanded();
                    true
                }
                Some(HistoryCell::Tool(ToolCell::Invocation(view)))
                    if view.has_command_output() =>
                {
                    view.toggle_command_expanded();
                    true
                }
                Some(HistoryCell::UserEcho(cell))
                    if super::super::user_echo_cell::would_fold(cell) =>
                {
                    cell.toggle_expanded();
                    true
                }
                _ => false,
            };
            if toggled {
                self.mark_dirty(index);
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod timing_tests {
    use super::*;
    use crate::llm::ChatStreamChunk;
    use crate::render::work_status::WorkStatus;
    use std::time::{Duration, Instant};

    /// 【思考计时】【轮次隔离回归】本轮工作已持续 115 秒时，新思考段不能沿用整轮耗时。
    /// 参数：无；返回：无，实时标题或定稿思考使用工作时钟时断言失败。
    #[test]
    fn reasoning_duration_excludes_work_before_first_reasoning_chunk() {
        let mut store = TranscriptStore::new(200);
        store.set_work_status(WorkStatus::Thinking);
        store.work_status_started = Instant::now().checked_sub(Duration::from_secs(115));
        store.push_chunk(&ChatStreamChunk {
            kind: ChatStreamKind::Reasoning,
            text: "短思考".into(),
        });
        let rendered = store
            .display_live_tail(
                100,
                &crate::render::transcript::store::TranscriptRenderOptions {
                    reasoning_mode: crate::render::ReasoningDisplayMode::Full,
                    tool_call_mode: crate::render::ToolCallDisplayMode::Full,
                },
            )
            .iter()
            .map(|line| line.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!rendered.contains("1m 55s"), "{rendered}");
        store.finalize_live_tail();
        let HistoryCell::Reasoning(cell) = &store.cells[0] else {
            panic!("expected reasoning cell")
        };
        assert!(
            cell.duration
                .is_some_and(|duration| duration < Duration::from_secs(1)),
            "{:?}",
            cell.duration
        );
        assert!(store.work_status_started.unwrap().elapsed() >= Duration::from_secs(115));
    }

    /// 【思考计时】【阶段冻结回归】工具参数到达时冻结思考，工具等待后开启下一段独立计时。
    /// 参数：无；返回：无，参数传输延长思考或下一段沿用旧计时时断言失败。
    #[test]
    fn tool_argument_progress_freezes_reasoning_and_next_segment_restarts() {
        let mut store = TranscriptStore::new(200);
        store.push_chunk(&ChatStreamChunk {
            kind: ChatStreamKind::Reasoning,
            text: "first".into(),
        });
        store.live_tail.as_mut().unwrap().started = Instant::now() - Duration::from_secs(2);
        store.push_tool_call_progress(&crate::llm::ToolCallStreamProgress {
            index: 0,
            name: Some("read_file".into()),
            edit_diff_counts: None,
            arguments_chars: 0,
            arguments_bytes: 0,
            arguments_preview: String::new(),
        });
        let duration = store
            .live_tail
            .as_ref()
            .unwrap()
            .reasoning_duration()
            .unwrap();
        store.live_tail.as_mut().unwrap().started = Instant::now() - Duration::from_secs(115);
        assert_eq!(
            store.live_tail.as_ref().unwrap().reasoning_duration(),
            Some(duration)
        );
        store.push_chunk(&ChatStreamChunk {
            kind: ChatStreamKind::Reasoning,
            text: "second".into(),
        });
        let HistoryCell::Reasoning(first) = &store.cells[0] else {
            panic!("expected first thought")
        };
        assert_eq!(first.source, "first");
        assert_eq!(first.duration, Some(duration));
        assert!(
            store
                .live_tail
                .as_ref()
                .unwrap()
                .reasoning_duration()
                .unwrap()
                < Duration::from_secs(1)
        );
        store.set_work_status(WorkStatus::WaitingExternal);
        let second = store.live_tail.as_ref().unwrap().reasoning_duration();
        store.live_tail.as_mut().unwrap().started = Instant::now() - Duration::from_secs(115);
        store.finalize_live_tail();
        let HistoryCell::Reasoning(cell) = &store.cells[1] else {
            panic!("expected second thought")
        };
        assert_eq!(cell.duration, second);
    }
}
