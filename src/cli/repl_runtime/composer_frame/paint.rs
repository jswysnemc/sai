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
        // 1. 【输入图片】【高度降级】辅助图片不能挤掉正文或遭到半幅裁剪
        if self.visual_lines(&layout, cols).len() > usize::from(height) {
            layout.image_strip = Default::default();
        }
        // 1. 【终端】【候选面板】按实际视口收缩引用或斜杠面板窗口，避免末尾选中项遭到裁剪
        if layout.mention_panel.is_visible() {
            let other_rows = self
                .visual_lines(&layout, cols)
                .len()
                .saturating_sub(usize::from(layout.mention_panel.height()));
            layout
                .mention_panel
                .limit_height(usize::from(height).saturating_sub(other_rows));
        } else if layout.slash_panel.is_visible() {
            let other_rows = self
                .visual_lines(&layout, cols)
                .len()
                .saturating_sub(usize::from(layout.slash_panel.height()));
            layout
                .slash_panel
                .limit_height(usize::from(height).saturating_sub(other_rows));
        }
        // 2. 【终端】【输入视口】小窗口优先保留光标周围正文，再裁剪辅助区域
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
        layout.first_visual_row += usize::from(layout.cursor_row_offset.saturating_sub(cursor));
        layout.styled_display_lines = visible;
        layout.cursor_row_offset = cursor;
        let (mut lines, row_meta) = self.visual_lines_with_meta(&layout, cols);
        let footer_rows = usize::from(KEY_HINT_ROWS)
            + usize::from(
                !layout.mention_panel.is_visible()
                    && !layout.slash_panel.is_visible()
                    && !layout.shell_hint.is_visible(),
            );
        let footer_start = lines.len().saturating_sub(footer_rows);
        let cursor_offset = self.top_lines(cols).len()
            + usize::from(CHROME_INPUT_PAD_ROWS + CHROME_INPUT_INNER_PAD_ROWS)
            + layout.image_strip.height()
            + usize::from(layout.cursor_row_offset);
        let hidden_rows = lines
            .len()
            .saturating_sub(usize::from(height))
            .min(cursor_offset);
        lines.drain(..hidden_rows);
        let mut completion_hits = Vec::new();
        let mut completion_start: Option<u16> = None;
        let mut completion_end: u16 = 0;
        for (offset, entry) in row_meta
            .into_iter()
            .skip(hidden_rows)
            .take(usize::from(height))
            .enumerate()
        {
            if let Some((kind, item_meta)) = entry {
                let row = top.saturating_add(offset as u16);
                completion_start.get_or_insert(row);
                completion_end = row.saturating_add(1);
                if let Some((index, disabled)) = item_meta {
                    completion_hits.push(CompletionHit {
                        row,
                        kind,
                        index,
                        disabled,
                    });
                }
            }
        }
        let completion_area = completion_start.map(|start| (start, completion_end));
        let footer_end = lines.len().min(usize::from(height));
        let footer_start = footer_start.saturating_sub(hidden_rows).min(footer_end);
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
        let mut signature = ComposerSignature {
            footer_actions: if footer_rows > usize::from(KEY_HINT_ROWS) && footer_start < footer_end
            {
                self.chrome
                    .footer_actions(&lines[footer_start], cols)
                    .into_iter()
                    .map(|(columns, action)| {
                        (top.saturating_add(footer_start as u16), columns, action)
                    })
                    .collect()
            } else {
                Vec::new()
            },
            images: layout.image_strip.placements(
                top.saturating_add(
                    (self.top_lines(cols).len()
                        + usize::from(CHROME_INPUT_PAD_ROWS + CHROME_INPUT_INNER_PAD_ROWS))
                    .saturating_sub(hidden_rows) as u16,
                ),
            ),
            footer_geometry: Some(InputGeometry::from_rendered_lines(
                &lines[footer_start..footer_end],
                cols,
                top.saturating_add(footer_start as u16),
            )),
            completion_hits,
            completion_area,
            input_geometry: Some({
                let hidden_input = hidden_rows
                    .saturating_sub(cursor_offset - usize::from(layout.cursor_row_offset));
                InputGeometry::new(
                    &self.input,
                    chrome_input_content_cols(cols),
                    layout.first_visual_row + hidden_input,
                    top + (cursor_offset - usize::from(layout.cursor_row_offset))
                        .saturating_sub(hidden_rows) as u16,
                    CHROME_INPUT_PREFIX_COLS,
                    layout
                        .styled_display_lines
                        .len()
                        .saturating_sub(hidden_input),
                )
            }),
            top,
            height,
            cols,
            lines,
            cursor_col,
            cursor_row,
            hidden_rows,
            panel_actions: self.panel_actions.clone(),
            queue_hits: self.queue_hits.clone(),
            panel_widths: self
                .panel_lines
                .iter()
                .map(|line| crate::cli::repl_text::visible_width(line).min(cols))
                .collect(),
        };
        if let Some((column, row)) = self.panel_pointer {
            if let Some((_, columns, _)) =
                signature
                    .footer_actions
                    .iter()
                    .find(|(candidate, columns, _)| {
                        *candidate == row && columns.contains(&usize::from(column))
                    })
            {
                let line = &mut signature.lines[usize::from(row - top)];
                *line = super::super::fullscreen::control_hover::tint_cols(
                    line,
                    columns.start,
                    columns.end,
                );
            }
        }
        if let Some(geometry) = &signature.input_geometry {
            if let Some(range) = self.selection.range(geometry) {
                for (offset, line) in signature.lines.iter_mut().enumerate() {
                    if let Some(columns) = geometry.columns(top + offset as u16, &range) {
                        *line = super::super::fullscreen::selection::highlight_cols(
                            line,
                            columns.start,
                            columns.end,
                        );
                    }
                }
            }
        }
        if let Some(geometry) = &signature.footer_geometry {
            if let Some(range) = self.selection.footer_range(geometry) {
                for (offset, line) in signature.lines.iter_mut().enumerate() {
                    if let Some(columns) = geometry.columns(top + offset as u16, &range) {
                        *line = super::super::fullscreen::selection::highlight_cols(
                            line,
                            columns.start,
                            columns.end,
                        );
                    }
                }
            }
        }
        if let Some((column, row)) = self
            .panel_pointer
            .filter(|(column, row)| signature.queue_action_at(*column, *row).is_some())
        {
            let offset = usize::from(row - top);
            let source = offset + hidden_rows;
            if let Some(hit) = self
                .queue_hits
                .iter()
                .find(|hit| hit.row == source && hit.columns.contains(&usize::from(column)))
            {
                signature.lines[offset] = super::super::fullscreen::control_hover::tint_cols(
                    &signature.lines[offset],
                    hit.columns.start,
                    hit.columns.end,
                );
            }
        }
        if let Some(index) = self
            .panel_pointer
            .and_then(|(column, row)| signature.panel_row_at(column, row))
            .filter(|index| self.panel_actions.iter().any(|(row, _)| row == index))
        {
            let line = &mut signature.lines[index - hidden_rows];
            let action = self
                .panel_actions
                .iter()
                .find(|(row, _)| *row == index)
                .map(|(_, action)| action);
            // 1. 【终端】【面板悬停】展开控件按实际状态显示箭头，导航条目只提亮原图标
            let text = if matches!(action, Some(PanelAction::Todo | PanelAction::AgentsToggle)) {
                super::super::fullscreen::guide_button::render(
                    line,
                    self.panel_expanded_rows.contains(&index),
                )
            } else {
                line.clone()
            };
            *line = super::super::fullscreen::hover::tint_line(&text);
        }
        if let Some((column, row)) = self.completion_pointer {
            if let Some(hit) = signature
                .completion_hit_at(column, row)
                .filter(|hit| !hit.disabled)
            {
                let offset = usize::from(hit.row.saturating_sub(top));
                if let Some(line) = signature.lines.get_mut(offset) {
                    *line = super::super::fullscreen::hover::tint_line(line);
                }
            }
        }
        let same_geometry =
            previous.filter(|old| old.top == top && old.height == height && old.cols == cols);
        let previous_lines = same_geometry.map(|old| old.lines.as_slice());
        let images_changed = previous.is_none_or(|old| old.images != signature.images);
        let images_repaint = images_changed || signature.image_rows_changed(previous_lines);
        if images_changed {
            if let Some(old) = previous {
                old.clear_images(output)?;
            }
        }
        // 3. 光标单独恢复；内容相同时不切换隐藏状态，也不重写静态行
        let changed = previous_lines != Some(signature.lines.as_slice()) || images_changed;
        if changed {
            queue!(output, Hide)?;
            paint_changed_rows(
                output,
                top,
                cols,
                &signature.lines,
                if images_changed { None } else { previous_lines },
            )?;
        }
        // 4. 布局改变后清理区域下方的旧内容，稳定布局只更新发生变化的行
        let end_row = top.saturating_add(height);
        if same_geometry.is_none() && end_row < viewport.size().rows {
            queue!(output, MoveTo(0, end_row), Clear(ClearType::FromCursorDown))?;
        }
        if images_repaint {
            signature.paint_images(output)?;
        }
        let cursor_changed =
            previous.is_none_or(|old| old.cursor_col != cursor_col || old.cursor_row != cursor_row);
        if changed || images_repaint || cursor_changed || same_geometry.is_none() {
            signature.restore_cursor(output)?;
        }
        Ok((cursor_row, signature))
    }

    /// 【终端】【输入绘制】组装完整视觉行，统一行数、样式与差异比较的数据来源。
    /// 参数: `layout` 为正文和补全面板布局，`cols` 为终端列数
    /// 返回: 按显示顺序排列的面板、输入和底栏
    fn visual_lines(&self, layout: &ComposerLayout, cols: usize) -> Vec<String> {
        self.visual_lines_with_meta(layout, cols).0
    }

    /// 【终端】【输入绘制】同时生成视觉行与补全面板行号元数据，保证点击与渲染零偏移。
    #[allow(clippy::type_complexity)]
    fn visual_lines_with_meta(
        &self,
        layout: &ComposerLayout,
        cols: usize,
    ) -> (
        Vec<String>,
        Vec<Option<(CompletionPanelKind, Option<(usize, bool)>)>>,
    ) {
        let mut lines = self.top_lines(cols);
        lines.extend((0..CHROME_INPUT_PAD_ROWS).map(|_| String::new()));
        lines.extend((0..CHROME_INPUT_INNER_PAD_ROWS).map(|_| self.input_padding(cols, true)));
        lines.extend(
            layout
                .image_strip
                .lines()
                .iter()
                .map(|line| self.input_row(ChromeInputPrefix::Continuation, line, cols)),
        );
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
            lines.push(self.input_row(prefix, segment, cols));
            first = false;
        }
        lines.extend((0..CHROME_INPUT_INNER_PAD_ROWS).map(|_| self.input_padding(cols, false)));
        let mut meta = vec![None; lines.len()];
        let mut hints = self.key_hints;
        let suppress_hint_row = if layout.mention_panel.is_visible() {
            let (panel_lines, panel_meta) = layout.mention_panel.rendered_rows_with_meta(cols);
            lines.extend(panel_lines);
            meta.extend(
                panel_meta
                    .into_iter()
                    .map(|item| Some((CompletionPanelKind::Mention, item))),
            );
            hints.mode = super::key_hints::ActiveBottomMode::Completion;
            false
        } else if layout.slash_panel.is_visible() {
            let (panel_lines, panel_meta) = layout.slash_panel.rendered_rows_with_meta(cols);
            lines.extend(panel_lines);
            meta.extend(
                panel_meta
                    .into_iter()
                    .map(|item| Some((CompletionPanelKind::Slash, item))),
            );
            if layout.slash_panel.has_header() {
                true
            } else {
                hints.mode = super::key_hints::ActiveBottomMode::Completion;
                false
            }
        } else if layout.shell_hint.is_visible() {
            lines.extend(layout.shell_hint.rendered_lines(cols));
            meta.resize(lines.len(), None);
            true
        } else {
            lines.push(self.chrome.footer_line(cols));
            meta.push(None);
            false
        };
        // 最后一行固定为按键提示：无顶栏的补全候选显示操作说明，有顶栏面板时仅保留确认提示
        lines.push(super::key_hints::render_key_hints(
            hints,
            self.input.is_empty(),
            suppress_hint_row,
            cols,
        ));
        meta.push(None);
        (lines, meta)
    }
}

impl ComposerSignature {
    /// 【终端】【光标恢复】正文更新后恢复输入光标，不重复绘制输入区。
    /// 参数：output 为本帧输出；返回：光标定位与显示结果。
    pub(in crate::cli::repl_runtime) fn restore_cursor<W: Write>(
        &self,
        output: &mut W,
    ) -> Result<()> {
        queue!(output, MoveTo(self.cursor_col, self.cursor_row), Show)?;
        Ok(())
    }
}
