use super::line::AnsiLine;
use super::spacing;
use super::store::TranscriptStore;
use super::{assistant_body, markdown_cell, reasoning_cell, TranscriptRenderOptions};
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
    #[cfg(test)]
    pub(crate) fn display_window(
        &mut self,
        width: usize,
        options: &TranscriptRenderOptions,
        min_rows: usize,
        max_start: usize,
    ) -> DisplayWindow {
        self.display_window_with_live_cap(width, options, min_rows, max_start, usize::MAX)
    }

    /// 渲染尾部窗口，并对临时 live 预览限制行数。
    ///
    /// 未闭合表格的列宽随后续行回溯变化，已渲染行一旦被真实滚动推入
    /// 原生 scrollback 就无法再修补，成为永久残留；这类临时预览截断为
    /// 尾部 `live_cap` 行。普通流式正文渲染稳定，**不截断**，
    /// 否则正文会被困在固定高度内反复重绘而无法向下增长。
    ///
    /// 参数:
    /// - `width`: 当前终端列数
    /// - `options`: transcript 渲染选项
    /// - `min_rows`: 窗口至少覆盖的行数
    /// - `max_start`: 窗口首行不得晚于该全局行号
    /// - `live_cap`: live 预览最多保留的尾部行数
    ///
    /// 返回:
    /// - 增量同步视图
    pub(crate) fn display_window_with_live_cap(
        &mut self,
        width: usize,
        options: &TranscriptRenderOptions,
        min_rows: usize,
        max_start: usize,
        live_cap: usize,
    ) -> DisplayWindow {
        // 子智能体视图：整个 transcript 切换为该子智能体的会话时间线
        if let super::store::TranscriptView::Subagent { id, label } = self.view.clone() {
            let lines = super::subagent_view::render_view_lines(
                &id,
                &label,
                width,
                self.live_animation_frame(),
            );
            let total = lines.len();
            let start = total.saturating_sub(min_rows).min(max_start).min(total);
            return DisplayWindow {
                total,
                start,
                lines: lines.into_iter().skip(start).collect(),
                // 视图内容整体可变：全窗口参与 diff
                dirty_from: start,
            };
        }
        let frame = self.live_animation_frame();
        let (live_full, transient) = self.display_live_tail_parts(width, options);
        // 仅临时结构（未闭合表格预览）截断到尾部 live_cap 行；
        // 稳定正文必须完整参与窗口，才能正常增长并滚入 scrollback
        let live_skip = if transient {
            live_full.len().saturating_sub(live_cap.max(1))
        } else {
            0
        };
        let mut live: Vec<AnsiLine> = live_full.into_iter().skip(live_skip).collect();
        // 1. 统计每个 cell 的行数（缓存命中时只读长度，不重新渲染）
        let mut counts = Vec::with_capacity(self.cells.len());
        let mut gap_before = vec![false; self.cells.len()];
        let mut cell_rows = 0usize;
        for (index, cell) in self.cells.iter().enumerate() {
            if index > 0 && spacing::needs_section_gap(&self.cells[index - 1], cell) {
                gap_before[index] = true;
                cell_rows += 1;
            }
            let count = self.cache.count_for(index, cell, width, options, frame);
            counts.push(count);
            cell_rows += count;
        }
        // 2. 定稿区最后一行若已是空行，丢掉 live 自己的前空行；正文后的工具 live 再补一块空行
        if let Some(last_index) = self.cells.len().checked_sub(1) {
            let last_lines =
                self.cache
                    .lines_for(last_index, &self.cells[last_index], width, options, frame);
            spacing::drop_duplicate_leading_blank(&mut live, last_lines.last());
            spacing::ensure_live_tool_gap(&mut live, self.cells.get(last_index));
        }
        let total = cell_rows + live.len();
        // 3. 脏行水位：有脏 cell 时取其起始行（含它前面的区块空行），否则从 live 区起点算起
        let dirty_from = match self.dirty_from_cell {
            Some(cell_index) => {
                let mut rows = 0usize;
                for index in 0..cell_index {
                    if gap_before[index] {
                        rows += 1;
                    }
                    rows += counts[index];
                }
                if gap_before.get(cell_index) == Some(&true) {
                    rows += 1;
                }
                rows
            }
            None => cell_rows,
        }
        .min(total);
        // 4. 窗口首行同时满足最小覆盖行数与调用方的追加/修补需求
        let start = total.saturating_sub(min_rows).min(max_start).min(cell_rows);
        // 5. 顺序拼出窗口行：跳过完全位于窗口上方的 cell，首个跨界 cell 截取尾部
        let mut lines = Vec::with_capacity(total - start);
        let mut offset = 0usize;
        for (index, count) in counts.iter().enumerate() {
            if gap_before[index] {
                if offset >= start {
                    lines.push(AnsiLine::new(String::new()));
                }
                offset += 1;
            }
            let end = offset + count;
            if end > start {
                let cell_lines =
                    self.cache
                        .lines_for(index, &self.cells[index], width, options, frame);
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
    #[cfg(test)]
    pub(crate) fn display_live_tail(
        &self,
        width: usize,
        options: &TranscriptRenderOptions,
    ) -> Vec<AnsiLine> {
        self.display_live_tail_parts(width, options).0
    }

    /// 渲染 live 尾部，并报告其中是否含随后续内容回溯变化的临时结构。
    ///
    /// 参数:
    /// - `width`: 当前终端列数
    /// - `options`: transcript 渲染选项
    ///
    /// 返回:
    /// - `(预换行 ANSI 行, 是否含临时结构)`
    fn display_live_tail_parts(
        &self,
        width: usize,
        options: &TranscriptRenderOptions,
    ) -> (Vec<AnsiLine>, bool) {
        let mut lines = Vec::new();
        let mut transient = false;
        let mut emitted_live_content = false;
        if let Some(tail) = &self.live_tail {
            // 【终端】【流式正文】1. 按内容类型选择渲染方式，避免宽度与折行分支漂移
            let mut rendered_lines = match tail.kind {
                ChatStreamKind::Content => {
                    let (rendered, open) =
                        crate::render::render_width::with_render_width(width, || {
                            markdown_cell::render_completed_parts(&tail.source)
                        });
                    transient = open;
                    // 【终端】【正文引导】2. Markdown 流式正文添加与定稿正文一致的引导区
                    assistant_body::display_lines(&rendered, width)
                }
                ChatStreamKind::Reasoning => {
                    let elapsed = self
                        .work_status_started
                        .map(|started| started.elapsed())
                        .unwrap_or_default();
                    let rendered = crate::render::render_width::with_render_width(width, || {
                        reasoning_cell::render_live(
                            &tail.source,
                            options.reasoning_mode,
                            self.live_animation_frame(),
                            elapsed,
                            tail.expanded,
                        )
                    });
                    if rendered.is_empty() {
                        Vec::new()
                    } else {
                        AnsiLine::wrap_block(&rendered, width)
                    }
                }
            };
            // 3. 丢掉折行带进来的首尾视觉空行，区块前距只保留下面这一行
            spacing::trim_leading_visual_blanks(&mut rendered_lines);
            spacing::trim_trailing_visual_blanks(&mut rendered_lines);
            // 与定稿 Markdown / Reasoning 一致：区块前空一行。
            // 以前只在 finalize 后由 cell.display_lines 插入，造成「工作时无空行、完成后突然空行」。
            if !rendered_lines.is_empty() {
                lines.push(AnsiLine::new(String::new()));
                if tail.kind == ChatStreamKind::Content {
                    emitted_live_content = true;
                }
            }
            lines.extend(rendered_lines);
        }
        let has_live_reasoning = self
            .live_tail
            .as_ref()
            .is_some_and(|tail| tail.kind == ChatStreamKind::Reasoning && !tail.source.is_empty());
        if let Some(tool_call) = &self.live_tool_call {
            // 编辑类 live 固定 Summary 一行（Write +N -M），与普通工具同行宽对齐
            let rendered = crate::render::render_width::with_render_width(width, || {
                super::tool_cell::render_live_call(
                    &tool_call.name,
                    &tool_call.arguments_preview,
                    options.tool_call_mode,
                )
            });
            if !rendered.is_empty() {
                let mut tool_lines = AnsiLine::wrap_block(&rendered, width);
                spacing::trim_trailing_visual_blanks(&mut tool_lines);
                // 流式正文与工具预览同在 live 区时，也要留下和定稿一样的一块空行
                if emitted_live_content {
                    spacing::ensure_blank_before(&mut lines);
                }
                lines.extend(tool_lines);
            }
        }
        if let Some(status) = self.work_status {
            if !has_live_reasoning {
                let elapsed = self
                    .work_status_started
                    .map(|started| started.elapsed())
                    .unwrap_or_default();
                let mut status_lines = AnsiLine::wrap_block(
                    &status.render_line(self.live_animation_frame(), elapsed),
                    width,
                );
                spacing::trim_trailing_visual_blanks(&mut status_lines);
                lines.extend(status_lines);
            }
        }
        (lines, transient)
    }
}
