use super::*;
use crate::render::terminal_rows::paint_changed_rows;

impl ComposerFrame {
    /// 【终端】【输入绘制】比较视觉行并输出变化，活动状态不能触发整块输入区清屏。
    ///
    /// 参数: `output` 为帧缓冲，`viewport` 为区域布局，`previous` 为上次绘制签名
    /// 返回: 输入光标行与本次绘制签名
    pub(in crate::cli::repl_runtime) fn draw_lines<W: Write>(
        &self,
        output: &mut W,
        viewport: &InlineViewport,
        previous: Option<&ComposerSignature>,
    ) -> Result<(u16, ComposerSignature)> {
        let cols = usize::from(viewport.size().cols);
        let top = viewport.composer_top();
        let height = viewport.composer_height();
        let mut layout = self.layout(cols);
        // 1. 【终端】【输入视口】小窗口优先保留光标周围正文，再裁剪辅助区域
        let overhead = self
            .visual_lines(&layout, cols)
            .len()
            .saturating_sub(layout.styled_display_lines.len());
        let available = usize::from(height).saturating_sub(overhead).max(1);
        let (visible, cursor) = super::input_viewport::select_rows(
            &layout.styled_display_lines,
            usize::from(layout.cursor_row_offset),
            available,
        );
        layout.styled_display_lines = visible;
        layout.cursor_row_offset = cursor;
        let mut lines = self.visual_lines(&layout, cols);
        let cursor_offset = self.panel_lines.len()
            + usize::from(CHROME_INPUT_PAD_ROWS + CHROME_INPUT_INNER_PAD_ROWS)
            + usize::from(layout.cursor_row_offset);
        let hidden_rows = lines
            .len()
            .saturating_sub(usize::from(height))
            .min(cursor_offset);
        lines.drain(..hidden_rows);
        lines.resize(usize::from(height), String::new());
        let cursor_col = layout
            .cursor_col
            .saturating_add(CHROME_INPUT_PREFIX_COLS as u16)
            .min(cols.saturating_sub(1) as u16);
        let cursor_row = top.saturating_add(
            cursor_offset
                .saturating_sub(hidden_rows)
                .min(usize::from(height.saturating_sub(1))) as u16,
        );
        let signature = ComposerSignature {
            top,
            height,
            cols,
            lines,
            cursor_col,
            cursor_row,
        };
        let same_geometry =
            previous.filter(|old| old.top == top && old.height == height && old.cols == cols);
        let previous_lines = same_geometry.map(|old| old.lines.as_slice());
        // 2. 光标单独恢复；内容相同时不切换隐藏状态，也不重写静态行
        if previous_lines != Some(signature.lines.as_slice()) {
            queue!(output, Hide)?;
            paint_changed_rows(output, top, cols, &signature.lines, previous_lines)?;
        }
        // 3. 布局改变后清理区域下方的旧内容，稳定布局只更新发生变化的行
        let end_row = top.saturating_add(height);
        if same_geometry.is_none() && end_row < viewport.size().rows {
            queue!(output, MoveTo(0, end_row), Clear(ClearType::FromCursorDown))?;
        }
        queue!(output, MoveTo(cursor_col, cursor_row), Show)?;
        Ok((cursor_row, signature))
    }

    /// 【终端】【输入绘制】组装完整视觉行，统一行数、样式与差异比较的数据来源。
    /// 参数: `layout` 为正文和补全面板布局，`cols` 为终端列数
    /// 返回: 按显示顺序排列的面板、输入和底栏
    fn visual_lines(&self, layout: &ComposerLayout, cols: usize) -> Vec<String> {
        let mut lines = self.panel_lines.clone();
        lines.extend((0..CHROME_INPUT_PAD_ROWS).map(|_| String::new()));
        lines.extend((0..CHROME_INPUT_INNER_PAD_ROWS).map(|_| chrome_input_pad_row(cols)));
        let first_prefix = if self.input.starts_with('!') {
            ChromeInputPrefix::Shell
        } else {
            ChromeInputPrefix::Message
        };
        let mut first = true;
        for segment in &layout.styled_display_lines {
            let prefix = if first {
                first_prefix
            } else {
                ChromeInputPrefix::Continuation
            };
            lines.push(chrome_input_row(prefix, segment, cols));
            first = false;
        }
        lines.extend((0..CHROME_INPUT_INNER_PAD_ROWS).map(|_| chrome_input_pad_row(cols)));
        let panel = if layout.mention_panel.is_visible() {
            lines.extend(layout.mention_panel.rendered_lines(cols));
            super::key_hints::HintPanel::Completion
        } else if layout.slash_panel.is_visible() {
            lines.extend(layout.slash_panel.rendered_lines(cols));
            super::key_hints::HintPanel::Completion
        } else if layout.shell_hint.is_visible() {
            lines.extend(layout.shell_hint.rendered_lines(cols));
            super::key_hints::HintPanel::Shell
        } else {
            lines.push(self.chrome.footer_line(cols));
            super::key_hints::HintPanel::None
        };
        // 最后一行固定为按键提示，随场景切换内容
        lines.push(super::key_hints::render_key_hints(
            self.key_hints,
            panel,
            self.input.trim().is_empty(),
            cols,
        ));
        lines
    }
}
