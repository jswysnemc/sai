use crate::control_commands::context_view::{self, ContextOverview};
use crate::i18n::text as t;
use crate::question_tui::OverlayPlacement;
use crate::render::panel_palette::{fg, PanelPalette};
use anyhow::Result;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind},
    execute, queue,
    style::Print,
    terminal::{Clear, ClearType},
};
use std::io::{self, Write};
use std::time::Duration;
use unicode_width::UnicodeWidthStr;

/// 【上下文】【面板交互】在正文下方打开只读面板，支持关闭、滚动和窗口缩放。
///
/// 参数:
/// - `view`: 上下文数据快照
/// - `placement`: 初始面板区域
/// - `resize`: 终端缩放时重新分配面板区域的回调
///
/// 返回:
/// - 面板展示与终端恢复结果
pub(super) fn show(
    view: &ContextOverview,
    mut placement: OverlayPlacement,
    mut resize: impl FnMut() -> Result<OverlayPlacement>,
) -> Result<()> {
    let was_raw = crossterm::terminal::is_raw_mode_enabled()?;
    if !was_raw {
        crossterm::terminal::enable_raw_mode()?;
    }
    let result = (|| {
        let _mouse = super::mouse_capture::MouseCapture::acquire()?;
        let mut click = super::mouse_menu::ClickTracker::default();
        execute!(io::stdout(), Hide)?;
        let mut offset = 0usize;
        let mut dirty = true;
        loop {
            if dirty {
                let layout = Layout::new(view, placement, offset);
                offset = layout.offset;
                layout.draw()?;
                dirty = false;
            }
            if !event::poll(Duration::from_millis(250))? {
                continue;
            }
            let previous_offset = offset;
            match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => match key.code {
                    KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Ok(())
                    }
                    KeyCode::Up | KeyCode::Char('k') => offset = offset.saturating_sub(1),
                    KeyCode::Down | KeyCode::Char('j') => offset = offset.saturating_add(1),
                    KeyCode::PageUp => offset = offset.saturating_sub(5),
                    KeyCode::PageDown | KeyCode::Char(' ') => offset = offset.saturating_add(5),
                    KeyCode::Home => offset = 0,
                    KeyCode::End => offset = usize::MAX,
                    _ => {}
                },
                Event::Resize(_, _) => {
                    click.clear();
                    placement = resize()?;
                    execute!(io::stdout(), Hide)?;
                    dirty = true;
                }
                Event::Mouse(mouse) => {
                    let layout = Layout::new(view, placement, offset);
                    let target = layout
                        .close
                        .as_ref()
                        .filter(|(row, columns)| {
                            *row == usize::from(mouse.row)
                                && columns.contains(&usize::from(mouse.column))
                        })
                        .map(|_| ());
                    if click.update(mouse, target).is_some() {
                        return Ok(());
                    }
                    if (placement.anchor_y..placement.anchor_y + placement.panel_lines)
                        .contains(&mouse.row)
                    {
                        match mouse.kind {
                            MouseEventKind::ScrollUp => offset = offset.saturating_sub(3),
                            MouseEventKind::ScrollDown => offset = offset.saturating_add(3),
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
            dirty |= offset != previous_offset;
        }
    })();
    let cleared = clear_panel(placement);
    let cursor = execute!(io::stdout(), Show).map_err(anyhow::Error::from);
    let raw = if !was_raw {
        crossterm::terminal::disable_raw_mode().map_err(anyhow::Error::from)
    } else {
        Ok(())
    };
    result.and(cleared).and(cursor).and(raw)
}

/// 【上下文】【面板清理】关闭时仅清除面板占用行，保留上方会话历史。
///
/// 参数:
/// - `placement`: 当前面板区域
///
/// 返回:
/// - 终端写入结果
fn clear_panel(placement: OverlayPlacement) -> Result<()> {
    let mut output = io::stdout();
    queue!(output, Print("\x1b[?2026h\x1b[0m"))?;
    for row in placement.anchor_y..placement.anchor_y + placement.panel_lines {
        queue!(output, MoveTo(0, row), Clear(ClearType::CurrentLine))?;
    }
    queue!(output, Print("\x1b[?2026l"))?;
    output.flush()?;
    Ok(())
}

struct Layout {
    close: Option<(usize, std::ops::Range<usize>)>,
    x: usize,
    y: usize,
    offset: usize,
    lines: Vec<String>,
}

impl Layout {
    /// 【上下文】【浮层布局】宽度不超过终端，正文超长时在底栏嵌入滚动进度与操作提示。
    ///
    /// 参数:
    /// - `view`: 上下文概览数据
    /// - `placement`: 覆盖区域尺寸
    /// - `offset`: 当前滚动偏移
    ///
    /// 返回:
    /// - 当前浮层布局
    fn new(view: &ContextOverview, placement: OverlayPlacement, offset: usize) -> Self {
        let palette = PanelPalette::current();
        let rule_fg = fg(palette.border);
        let muted_fg = fg(palette.muted);
        let cols = usize::from(placement.cols);
        let width = cols.saturating_sub(6).clamp(1, 114).min(cols.max(1));
        let inset = 2.min(width.saturating_sub(1) / 2);
        let content_width = width.saturating_sub(inset * 2).max(1);
        let mut content = context_view::render(view, content_width, true);
        let mut header = content.remove(0);
        let close = format!("Esc {}", t("Close", "关闭"));
        let used = crate::render::ansi_clip::ansi_visible_width(&header)
            + UnicodeWidthStr::width(close.as_str());
        let close_hit = (used + 2 <= content_width).then(|| {
            (
                usize::from(placement.anchor_y) + 1,
                inset + content_width - UnicodeWidthStr::width(close.as_str())
                    ..inset + content_width,
            )
        });
        if used + 2 <= content_width {
            header.push_str(&format!(
                "{}{muted_fg}{close}\x1b[0m",
                " ".repeat(content_width - used)
            ));
        }
        let height = usize::from(placement.panel_lines).max(1);
        let body_height = height.saturating_sub(3);
        let total_body = content.len();
        let max_offset = total_body.saturating_sub(body_height);
        let offset = offset.min(max_offset);

        // 1. 【上下文】【底栏滚动提示】正文可滚动时在底边分隔线中嵌入位置与滚动提示
        let top_rule = format!("{rule_fg}{}\x1b[0m", "─".repeat(width));
        let bottom_rule = if max_offset > 0 && width >= 10 {
            let shown_end = (offset + body_height).min(total_body);
            let raw_hint = format!(
                " {shown_end}/{total_body} · ↑↓ {} · Esc {} ",
                t("scroll", "滚动"),
                t("close", "关闭")
            );
            let clipped = crate::render::clip_to_width(&raw_hint, width.saturating_sub(2), "…");
            let hint_w = UnicodeWidthStr::width(clipped.as_str());
            let dashes = width.saturating_sub(hint_w);
            let left = dashes / 2;
            format!(
                "{rule_fg}{}\x1b[0m{muted_fg}{clipped}\x1b[0m{rule_fg}{}\x1b[0m",
                "─".repeat(left),
                "─".repeat(dashes - left)
            )
        } else {
            top_rule.clone()
        };

        let mut lines = vec![header];
        lines.extend(content.into_iter().skip(offset).take(body_height));
        let mut lines = lines
            .into_iter()
            .map(|line| {
                let padded = format!("{}{line}", " ".repeat(inset));
                if crate::render::ansi_clip::ansi_visible_width(&padded) > cols {
                    crate::render::ansi_clip::clip_ansi_to_width(&padded, cols, "")
                } else {
                    padded
                }
            })
            .collect::<Vec<_>>();
        lines.insert(0, top_rule);
        lines.push(bottom_rule);
        lines.truncate(height);
        Self {
            close: close_hit,
            x: 0,
            y: usize::from(placement.anchor_y),
            offset,
            lines,
        }
    }

    /// 【上下文】【浮层绘制】整行清除底层内容后绘制浮层，保持默认透明终端底色。
    ///
    /// 参数:
    /// - `self`: 当前布局
    ///
    /// 返回:
    /// - 绘制结果
    fn draw(&self) -> Result<()> {
        let mut output = io::stdout();
        queue!(output, Print("\x1b[?2026h"))?;
        for (row, line) in self.lines.iter().enumerate() {
            let row = (self.y + row) as u16;
            // 1. 【上下文】【逐行绘制】先清除整行避免两侧残留底层文字，再应用终端对比度校正
            let corrected = crate::render::readable_terminal_text(line);
            queue!(
                output,
                MoveTo(0, row),
                Clear(ClearType::CurrentLine),
                MoveTo(self.x as u16, row),
                Print(format!("{corrected}\x1b[0m"))
            )?;
        }
        queue!(output, Print("\x1b[?2026l"))?;
        output.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【上下文】【浮层边界测试】极窄、矮屏和普通终端都不越界，矮屏可滚动时底栏展示滚动提示。
    #[test]
    fn overlay_stays_inside_terminal_and_shows_scroll_hint() {
        for (cols, rows) in [(1, 1), (12, 6), (40, 20), (100, 30)] {
            let placement = OverlayPlacement::inline(cols, rows, 2);
            let layout = Layout::new(&ContextOverview::default(), placement, 0);
            assert!(layout.y + layout.lines.len() <= usize::from(rows));
            assert!(layout.lines.iter().all(|line| layout.x
                + crate::render::ansi_clip::ansi_visible_width(line)
                <= usize::from(cols)));
        }
        let short_scrollable = OverlayPlacement::inline(80, 6, 0);
        let layout = Layout::new(&ContextOverview::default(), short_scrollable, 0);
        let bottom_plain = crate::cli::repl_text::strip_terminal_control_sequences(
            layout.lines.last().unwrap(),
        );
        assert!(
            bottom_plain.contains("↑↓"),
            "可滚动上下文面板底栏应包含滚动提示: {bottom_plain}"
        );
    }
}
