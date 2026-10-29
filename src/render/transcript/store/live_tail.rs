use super::{HistoryCell, LiveTail};
use crate::llm::ChatStreamKind;
use std::time::{Duration, Instant};

impl LiveTail {
    /// 【思考计时】【片段创建】首次收到本段内容时独立计时，不引用工作或动画时钟。
    /// 参数：kind 为内容类型，source 为首个片段；返回：独立计时的流式尾部。
    pub(super) fn new(kind: ChatStreamKind, source: String) -> Self {
        Self {
            kind,
            started: Instant::now(),
            finished: None,
            source,
            expanded: false,
            table_layouts: Default::default(),
            markdown_cache: Default::default(),
        }
    }

    /// 【思考计时】【当前时长】接收思考期间增长，切换到工具或正文后返回已冻结时长。
    /// 参数：无；返回：当前思考段耗时，正文尾部返回空值。
    pub(in crate::render::transcript) fn reasoning_duration(&self) -> Option<Duration> {
        (self.kind == ChatStreamKind::Reasoning)
            .then(|| self.finished.unwrap_or_else(|| self.started.elapsed()))
    }

    /// 【思考计时】【结束边界】首次离开思考状态时冻结时长，重复通知不延长计时。
    /// 参数：无；返回：无。
    pub(super) fn finish_reasoning(&mut self) {
        if self.kind == ChatStreamKind::Reasoning {
            self.finished.get_or_insert_with(|| self.started.elapsed());
        }
    }

    /// 【终端】【片段定稿】转换为历史单元，思考保留本段耗时和展开状态。
    /// 参数：self 为待定稿尾部；返回：正文或思考历史单元。
    pub(super) fn into_cell(self) -> HistoryCell {
        match self.kind {
            ChatStreamKind::Content => {
                HistoryCell::Markdown(crate::render::transcript::markdown_cell::MarkdownCell {
                    source: self.source,
                    table_layouts: self.table_layouts,
                })
            }
            ChatStreamKind::Reasoning => {
                let duration = self.reasoning_duration();
                let mut cell =
                    crate::render::transcript::reasoning_cell::ReasoningCell::new(self.source);
                cell.expanded = self.expanded;
                cell.duration = duration;
                HistoryCell::Reasoning(cell)
            }
        }
    }
}
