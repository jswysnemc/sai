//! 输入条样式绘制；两种外观共享折行、光标与鼠标命中几何。

use super::*;
use crate::config::InputStyle;

impl ComposerFrame {
    /// 【输入样式】【快照设置】参数：style 为外观；返回：无
    pub(in crate::cli::repl_runtime) fn set_input_style(&mut self, style: InputStyle) {
        self.input_style = style;
    }

    /// 【输入样式】【正文绘制】参数：prefix 为行前缀，content 为正文，cols 为宽度；返回：定宽输入行
    pub(super) fn input_row(
        &self,
        prefix: ChromeInputPrefix,
        content: &str,
        cols: usize,
    ) -> String {
        match self.input_style {
            InputStyle::Panel => chrome_input_row(prefix, content, cols),
            InputStyle::Separator => {
                let text = crate::render::ansi_clip::clip_ansi_to_width(
                    content,
                    chrome_input_content_cols(cols),
                    "",
                );
                let row = format!("{}{text}", prefix.render());
                format!(
                    "\x1b[49m{row}{}\x1b[0m",
                    " ".repeat(cols.saturating_sub(crate::cli::repl_text::visible_width(&row)))
                )
            }
        }
    }

    /// 【输入样式】【边界绘制】参数：cols 为宽度，top 为顶部标识；返回：面板内边距或顶部细分割线
    pub(super) fn input_padding(&self, cols: usize, top: bool) -> String {
        match self.input_style {
            InputStyle::Panel => chrome_input_pad_row(cols),
            InputStyle::Separator if top => format!(
                "\x1b[49m{}{}\x1b[0m",
                crate::render::panel_palette::fg(
                    crate::render::panel_palette::PanelPalette::current().border
                ),
                "─".repeat(cols)
            ),
            InputStyle::Separator => " ".repeat(cols),
        }
    }
}
