use super::line::AnsiLine;
use super::store::TranscriptStore;
use super::{markdown_cell, reasoning_cell, TranscriptRenderOptions};
use crate::llm::ChatStreamKind;

/// 一次增量同步所需的 transcript 视图数据。
///
/// `lines` 覆盖全局行号 `[start, total)`；`dirty_from` 是自上次同步以来
/// 第一处可能变化的全局行号，之前的行保证与上次渲染完全一致。
pub(crate) struct DisplayWindow {
    /// 当前 transcript 的总视觉行数
    pub(crate) total: usize,
    /// 窗口首行的全局行号
    pub(crate) start: usize,
    /// 窗口内的预换行 ANSI 行
    pub(crate) lines: Vec<AnsiLine>,
    /// 第一处可能变化的全局行号
    pub(crate) dirty_from: usize,
}

impl DisplayWindow {
    /// 按全局行号取窗口内的行。
    ///
    /// 参数:
    /// - `row`: 全局行号
    ///
    /// 返回:
    /// - 窗口覆盖该行时返回行内容
    pub(crate) fn line_at(&self, row: usize) -> Option<&AnsiLine> {
        row.checked_sub(self.start)
            .and_then(|offset| self.lines.get(offset))
    }
}

impl TranscriptStore {
    /// 渲染当前 transcript 的尾部窗口与总行数。
    ///
    /// 参数:
    /// - `width`: 当前终端列数
    /// - `options`: transcript 渲染选项
    /// - `min_rows`: 窗口至少覆盖的行数
    /// - `max_start`: 窗口首行不得晚于该全局行号（保证追加与修补行都在窗口内）
    ///
    /// 返回:
    /// - 增量同步视图
    pub(crate) fn display_window(
        &mut self,
        width: usize,
        options: &TranscriptRenderOptions,
        min_rows: usize,
        max_start: usize,
    ) -> DisplayWindow {
        let live = self.display_live_tail(width, options);
        // 1. 统计每个 cell 的行数（缓存命中时只读长度，不重新渲染）
        let mut counts = Vec::with_capacity(self.cells.len());
        let mut cell_rows = 0usize;
        for (index, cell) in self.cells.iter().enumerate() {
            let count = self.cache.count_for(index, cell, width, options);
            counts.push(count);
            cell_rows += count;
        }
        let total = cell_rows + live.len();
        // 2. 脏行水位：有脏 cell 时取其起始行，否则从 live 区起点算起
        let dirty_from = match self.dirty_from_cell {
            Some(cell_index) => counts.iter().take(cell_index).sum::<usize>(),
            None => cell_rows,
        }
        .min(total);
        // 3. 窗口首行同时满足最小覆盖行数与调用方的追加/修补需求
        let start = total.saturating_sub(min_rows).min(max_start).min(cell_rows);
        // 4. 顺序拼出窗口行：跳过完全位于窗口上方的 cell，首个跨界 cell 截取尾部
        let mut lines = Vec::with_capacity(total - start);
        let mut offset = 0usize;
        for (index, count) in counts.iter().enumerate() {
            let end = offset + count;
            if end > start {
                let cell_lines = self
                    .cache
                    .lines_for(index, &self.cells[index], width, options);
                let skip = start.saturating_sub(offset);
                lines.extend(cell_lines.into_iter().skip(skip));
            }
            offset = end;
        }
        lines.extend(live);
        DisplayWindow {
            total,
            start,
            lines,
            dirty_from,
        }
    }

    /// 渲染定稿 cell 与 live 尾部的最后若干行（测试观察用）。
    ///
    /// 参数:
    /// - `width`: 当前终端列数
    /// - `options`: transcript 渲染选项
    ///
    /// 返回:
    /// - row cap 范围内的预换行 ANSI 行
    #[cfg(test)]
    pub(crate) fn display_tail(
        &mut self,
        width: usize,
        options: &TranscriptRenderOptions,
    ) -> Vec<AnsiLine> {
        let row_cap = self.row_cap;
        self.display_window(width, options, row_cap, usize::MAX)
            .lines
    }

    /// 渲染当前 live 尾部（流式文本、工具参数预览与工作状态）。
    ///
    /// 工作状态行放在 live 区底部：收敛移除时只收缩尾行，
    /// 不会使上方流式文本整体位移。
    ///
    /// 参数:
    /// - `width`: 当前终端列数
    /// - `options`: transcript 渲染选项
    ///
    /// 返回:
    /// - 当前 live 尾部的预换行 ANSI 行
    pub(crate) fn display_live_tail(
        &self,
        width: usize,
        options: &TranscriptRenderOptions,
    ) -> Vec<AnsiLine> {
        let mut lines = Vec::new();
        // 有思考内容时只显示 reasoning 动效，不再叠一层 working/thinking 文案
        let has_live_reasoning = self
            .live_tail
            .as_ref()
            .is_some_and(|tail| tail.kind == ChatStreamKind::Reasoning && !tail.source.is_empty());
        if let Some(tail) = &self.live_tail {
            let rendered = match tail.kind {
                ChatStreamKind::Content => markdown_cell::render_completed(&tail.source),
                // reasoning 在定稿前显示节流的字符计数与跳动标记，结束后再按配置完整固化
                ChatStreamKind::Reasoning => {
                    let elapsed = self
                        .work_status_started
                        .map(|started| started.elapsed())
                        .unwrap_or_default();
                    reasoning_cell::render_live(
                        &tail.source,
                        options.reasoning_mode,
                        self.live_animation_frame,
                        elapsed,
                    )
                }
            };
            if !rendered.is_empty() {
                lines.extend(AnsiLine::wrap_block(&rendered, width));
            }
        }
        if let Some(tool_call) = &self.live_tool_call {
            let rendered = super::tool_cell::render_live_call(
                &tool_call.name,
                &tool_call.arguments_preview,
                options.tool_call_mode,
            );
            if !rendered.is_empty() {
                lines.extend(AnsiLine::wrap_block(&rendered, width));
            }
        }
        if let Some(status) = self.work_status {
            if !has_live_reasoning {
                let elapsed = self
                    .work_status_started
                    .map(|started| started.elapsed())
                    .unwrap_or_default();
                lines.extend(AnsiLine::wrap_block(
                    &super::work_status_cell::render(status, self.live_animation_frame, elapsed),
                    width,
                ));
            }
        }
        lines
    }
}
