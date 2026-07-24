use super::{is_table_separator, render_table};
use crate::render::markdown_inline::render_table_cell_content;
use crate::render::streaming_replace::{clear_rendered_rows, raw_visual_rows, rendered_visual_rows};

/// Markdown 表格在不同输出表面中的预览策略。
#[derive(Clone, Copy)]
enum PreviewMode {
    /// CLI 实时终端：确认后逐行清屏并以最新列宽重绘整表。
    ReplaceTerminalRows,
    /// 仅缓冲，结束时一次输出最终表格（history / 稳定重放）。
    StableFinal,
    /// 全量重绘表面（TUI live）：缓冲；调用方通过 snapshot 取当前最优渲染。
    Snapshot,
}

/// Markdown 表格流式替换状态。
pub(crate) struct StreamingTable {
    lines: Vec<String>,
    /// 尚未确认表格时，已写出的原始 Markdown 视觉行数。
    raw_visual_rows: usize,
    /// 确认后最近一次预览表格占用的视觉行数（CLI 清屏用）。
    preview_visual_rows: usize,
    preview_mode: PreviewMode,
}

impl StreamingTable {
    /// 创建使用终端光标回退替换的流式表格。
    ///
    /// 返回:
    /// - 普通 CLI 使用的表格状态
    pub(crate) fn new() -> Self {
        Self {
            lines: Vec::new(),
            raw_visual_rows: 0,
            preview_visual_rows: 0,
            preview_mode: PreviewMode::ReplaceTerminalRows,
        }
    }

    /// 创建稳定重放表格状态。
    ///
    /// 表格在闭合前保持缓冲，结束后输出按全表列宽计算的最终表格。
    ///
    /// 返回:
    /// - 不包含光标回退序列的表格状态
    pub(crate) fn new_stable() -> Self {
        Self {
            lines: Vec::new(),
            raw_visual_rows: 0,
            preview_visual_rows: 0,
            preview_mode: PreviewMode::StableFinal,
        }
    }

    /// 创建全量重绘用的表格状态（TUI live 等）。
    ///
    /// 不输出光标控制序列；未结束表格通过 `snapshot` 按当前行集合重算列宽。
    ///
    /// 返回:
    /// - snapshot 模式表格状态
    pub(crate) fn new_source_preview() -> Self {
        Self {
            lines: Vec::new(),
            raw_visual_rows: 0,
            preview_visual_rows: 0,
            preview_mode: PreviewMode::Snapshot,
        }
    }

    /// 判断当前是否缓存了表格候选行。
    ///
    /// 返回:
    /// - 是否存在候选行
    pub(crate) fn is_active(&self) -> bool {
        !self.lines.is_empty()
    }

    /// 判断当前候选行是否已经确认构成表格。
    ///
    /// 返回:
    /// - 第二行是否为 Markdown 表格分隔行
    pub(crate) fn is_confirmed(&self) -> bool {
        self.lines
            .get(1)
            .map(String::as_str)
            .is_some_and(is_table_separator)
    }

    /// 推入表格候选行并返回即时预览。
    ///
    /// 参数:
    /// - `line`: 当前 Markdown 行
    ///
    /// 返回:
    /// - 当前输出表面应立即展示的文本（可能含清屏重绘）
    pub(crate) fn push_line(&mut self, line: &str) -> String {
        let was_confirmed = self.is_confirmed();
        self.lines.push(line.to_string());
        let now_confirmed = self.is_confirmed();

        match self.preview_mode {
            // 1. CLI：未确认前输出原文；确认后（含刚确认）按全表列宽清屏重绘
            PreviewMode::ReplaceTerminalRows => {
                if !now_confirmed {
                    self.raw_visual_rows += raw_visual_rows(line);
                    return format!("{line}\n");
                }
                // 刚确认时也需清掉已输出的原文行；后续行清掉上一帧表格预览
                let clear_existing = was_confirmed || self.raw_visual_rows > 0 || self.preview_visual_rows > 0;
                self.redraw_cli_preview(clear_existing)
            }
            // 2. 稳定/快照：只缓冲，由 finish/snapshot 输出
            PreviewMode::StableFinal | PreviewMode::Snapshot => String::new(),
        }
    }

    /// 结束当前表格并返回最终渲染。
    ///
    /// 返回:
    /// - 确认表格的替换/最终文本；非表格时按模式恢复原文或保持已输出原文
    pub(crate) fn finish(&mut self) -> String {
        let output = match self.preview_mode {
            PreviewMode::ReplaceTerminalRows if self.is_confirmed() => self.redraw_cli_preview(true),
            PreviewMode::ReplaceTerminalRows => String::new(),
            PreviewMode::StableFinal | PreviewMode::Snapshot if self.is_confirmed() => {
                render_table(&self.lines, render_table_cell_content)
            }
            PreviewMode::StableFinal | PreviewMode::Snapshot => self.render_raw_source(),
        };
        self.lines.clear();
        self.raw_visual_rows = 0;
        self.preview_visual_rows = 0;
        output
    }

    /// 非破坏性预览当前缓冲（供 TUI 全量重绘在未闭合表格时使用）。
    ///
    /// 返回:
    /// - 已确认：按当前行集合重算列宽后的表格
    /// - 未确认：原始 Markdown 候选行
    /// - 无缓冲：空串
    pub(crate) fn snapshot(&self) -> String {
        if self.lines.is_empty() {
            return String::new();
        }
        if self.is_confirmed() {
            render_table(&self.lines, render_table_cell_content)
        } else {
            self.render_raw_source()
        }
    }

    /// CLI 模式下清除旧预览并以最新列宽重绘整表。
    ///
    /// 参数:
    /// - `clear_existing_preview`: 是否清除上一帧预览/原文占用行
    ///
    /// 返回:
    /// - 清屏序列 + 最新表格文本
    fn redraw_cli_preview(&mut self, clear_existing_preview: bool) -> String {
        let mut output = String::new();
        if clear_existing_preview {
            // 1. 优先清已渲染表格预览；若刚从原文切到确认表，清 raw 行
            let rows = if self.preview_visual_rows > 0 {
                self.preview_visual_rows
            } else {
                self.raw_visual_rows
            };
            output.push_str(&clear_rendered_rows(rows));
        }
        let table = render_table(&self.lines, render_table_cell_content);
        self.preview_visual_rows = rendered_visual_rows(&table);
        self.raw_visual_rows = 0;
        output.push_str(&table);
        output
    }

    /// 将尚未确认成表格的候选行恢复为原始 Markdown。
    ///
    /// 返回:
    /// - 每行保留换行符的原始候选文本
    fn render_raw_source(&self) -> String {
        self.lines.iter().map(|line| format!("{line}\n")).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn streams_raw_rows_before_table_is_confirmed() {
        let mut table = StreamingTable::new();

        assert_eq!(table.push_line("| a | b |"), "| a | b |\n");
        // 分隔行到达后确认表格，应清掉原文并绘出带边框的表
        let confirmed = table.push_line("| - | - |");
        assert!(confirmed.contains('\u{250c}') || confirmed.contains('┌'));
        assert!(confirmed.contains("\x1b[1A\r\x1b[2K"));
    }

    #[test]
    fn progressive_rows_redraw_with_latest_widths() {
        let mut table = StreamingTable::new();

        table.push_line("| 软件 | 命令 |");
        table.push_line("|---|---|");
        let first = table.push_line("| Arch | `pacman` |");
        assert!(first.contains('┌'));
        let second = table.push_line("| Neovim | `sudo pacman -S neovim` |");
        // 1. 新行触发清屏重绘
        assert!(second.contains("\x1b[1A\r\x1b[2K"));
        // 2. 列宽吸收更长单元格
        assert!(second.contains("sudo pacman -S neovim"));
    }

    #[test]
    fn replaces_preview_with_final_table_on_finish() {
        let mut table = StreamingTable::new();

        table.push_line("| 软件 | 命令 |");
        table.push_line("|---|---|");
        table.push_line("| Neovim | `sudo pacman -S neovim` |");
        let output = table.finish();

        assert!(output.contains("sudo pacman -S neovim"));
        assert!(output.contains('┌'));
    }

    #[test]
    fn snapshot_mode_buffers_and_previews_confirmed_table() {
        let mut table = StreamingTable::new_source_preview();

        assert!(table.push_line("| a | b |").is_empty());
        assert!(table.push_line("| - | - |").is_empty());
        assert!(table.push_line("| 1 | 2 |").is_empty());
        let snap = table.snapshot();
        assert!(snap.contains('┌'));
        assert!(snap.contains('1'));
        let finished = table.finish();
        assert!(finished.contains('┌'));
    }

    #[test]
    fn stable_table_emits_only_the_calculated_result() {
        let mut table = StreamingTable::new_stable();

        assert!(table.push_line("| a | b |").is_empty());
        assert!(table.push_line("| - | - |").is_empty());
        assert!(table.push_line("| 1 | 2 |").is_empty());
        let output = table.finish();

        assert!(output.contains('┌'));
        assert!(!output.contains("| a | b |"));
    }

    #[test]
    fn stable_table_restores_unconfirmed_candidate_rows() {
        let mut table = StreamingTable::new_stable();

        assert!(table.push_line("| note |").is_empty());

        assert_eq!(table.finish(), "| note |\n");
    }
}
