use super::{is_table_separator, render_table};
use crate::render::markdown_inline::render_table_cell_content;
use crate::render::streaming_replace::{clear_rendered_rows, raw_visual_rows};

/// Markdown 表格在不同输出表面中的预览策略。
#[derive(Clone, Copy)]
enum PreviewMode {
    ReplaceTerminalRows,
    SourcePreview,
    StableFinal,
}

/// Markdown 表格流式替换状态。
pub(crate) struct StreamingTable {
    lines: Vec<String>,
    raw_visual_rows: usize,
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
            preview_mode: PreviewMode::ReplaceTerminalRows,
        }
    }

    /// 创建 source-backed 表格状态。
    ///
    /// REPL 在表格未结束时展示原始 Markdown，结束后由可变尾部整体替换为最终表格。
    ///
    /// 返回:
    /// - 不包含光标回退序列的表格状态
    pub(crate) fn new_stable() -> Self {
        Self {
            lines: Vec::new(),
            raw_visual_rows: 0,
            preview_mode: PreviewMode::StableFinal,
        }
    }

    /// 创建 source-backed 实时预览状态。
    ///
    /// 该模式只展示原始 Markdown，最终 history cell 会使用稳定模式重新计算表格。
    ///
    /// 返回:
    /// - 不包含终端回退序列的原文预览状态
    pub(crate) fn new_source_preview() -> Self {
        Self {
            lines: Vec::new(),
            raw_visual_rows: 0,
            preview_mode: PreviewMode::SourcePreview,
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
    /// - 当前输出表面应立即展示的原始行
    pub(crate) fn push_line(&mut self, line: &str) -> String {
        self.lines.push(line.to_string());
        self.raw_visual_rows += raw_visual_rows(line);
        match self.preview_mode {
            PreviewMode::ReplaceTerminalRows | PreviewMode::SourcePreview => format!("{line}\n"),
            PreviewMode::StableFinal => String::new(),
        }
    }

    /// 结束当前表格并返回最终渲染。
    ///
    /// 返回:
    /// - 确认表格的替换文本，非表格返回空
    pub(crate) fn finish(&mut self) -> String {
        let output = match self.preview_mode {
            PreviewMode::SourcePreview => String::new(),
            PreviewMode::ReplaceTerminalRows | PreviewMode::StableFinal if self.is_confirmed() => {
                self.render_current()
            }
            PreviewMode::StableFinal => self.render_raw_source(),
            PreviewMode::ReplaceTerminalRows => String::new(),
        };
        self.lines.clear();
        self.raw_visual_rows = 0;
        output
    }

    /// 按全部已知行计算列宽并生成最终表格。
    ///
    /// 返回:
    /// - 普通 CLI 包含清除序列，REPL 仅包含最终表格
    fn render_current(&self) -> String {
        let mut output = match self.preview_mode {
            PreviewMode::ReplaceTerminalRows => clear_rendered_rows(self.raw_visual_rows),
            PreviewMode::SourcePreview | PreviewMode::StableFinal => String::new(),
        };
        output.push_str(&render_table(&self.lines, render_table_cell_content));
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
    fn streams_raw_rows_before_final_render() {
        let mut table = StreamingTable::new();

        assert_eq!(table.push_line("| a | b |"), "| a | b |\n");
        assert_eq!(table.push_line("| - | - |"), "| - | - |\n");
    }

    #[test]
    fn replaces_raw_rows_with_rendered_table_on_finish() {
        let mut table = StreamingTable::new();

        table.push_line("| 软件 | 命令 |");
        table.push_line("|---|---|");
        table.push_line("| Neovim | `sudo pacman -S neovim` |");
        let output = table.finish();

        assert!(output.starts_with("\x1b[1A\r\x1b[2K"));
        assert!(output.contains("sudo pacman -S neovim"));
    }

    #[test]
    fn source_backed_table_previews_raw_rows_without_cursor_replacement() {
        let mut table = StreamingTable::new_source_preview();

        assert_eq!(table.push_line("| a | b |"), "| a | b |\n");
        assert_eq!(table.push_line("| - | - |"), "| - | - |\n");
        table.push_line("| 1 | 2 |");
        let output = table.finish();

        assert!(output.is_empty());
        assert!(!output.contains("\x1b[1A"));
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
