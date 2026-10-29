use crate::control_commands::context_view::{self, ContextOverview};
use crate::question_tui::OverlayPlacement;
use anyhow::Result;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute, queue,
    style::Print,
    terminal::{Clear, ClearType},
};
use std::io::{self, Write};
use std::time::Duration;

/// 浮层上下边界线样式，弱化显示，不铺设整块底色。
const RULE: &str = "\x1b[2m";

/// 【上下文】【面板交互】在正文下方打开只读面板，支持关闭、滚动和窗口缩放。
/// 参数：数据快照、初始区域和重新分配区域的回调；返回：展示结果。
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
                    KeyCode::Up => offset = offset.saturating_sub(1),
                    KeyCode::Down => offset = offset.saturating_add(1),
                    KeyCode::PageUp => offset = offset.saturating_sub(5),
                    KeyCode::PageDown => offset = offset.saturating_add(5),
                    _ => {}
                },
                Event::Resize(_, _) => {
                    placement = resize()?;
                    execute!(io::stdout(), Hide)?;
                    dirty = true;
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

/// 【上下文】【面板清理】关闭时仅清除面板占用行，避免遗留图表并保留上方历史。
/// 参数：`placement` 为当前面板区域；返回：终端写入结果。
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
    x: usize,
    y: usize,
    offset: usize,
    lines: Vec<String>,
}

impl Layout {
    /// 【上下文】【浮层布局】宽度不超过终端，标题固定，正文过高时允许滚动。
    /// 参数: 数据、终端宽高与滚动量；返回: 当前浮层布局。
    fn new(view: &ContextOverview, placement: OverlayPlacement, offset: usize) -> Self {
        let cols = usize::from(placement.cols);
        let width = cols.saturating_sub(6).clamp(1, 114);
        let inset = 2.min(width.saturating_sub(1) / 2);
        let content_width = width.saturating_sub(inset * 2).max(1);
        let mut content = context_view::render(view, content_width, true);
        let mut header = content.remove(0);
        let close = format!("Esc {}", crate::i18n::text("Close", "关闭"));
        let used = crate::render::ansi_clip::ansi_visible_width(&header)
            + unicode_width::UnicodeWidthStr::width(close.as_str());
        if used + 2 <= content_width {
            header.push_str(&format!("{}{close}", " ".repeat(content_width - used)));
        }
        let height = usize::from(placement.panel_lines).max(1);
        let body_height = height.saturating_sub(3);
        let offset = offset.min(content.len().saturating_sub(body_height));
        // 1. 【上下文】【浮层边界】上下各一条弱化分隔线界定浮层范围，正文沿用终端底色
        let rule = format!("{RULE}{}\x1b[0m", "─".repeat(width));
        let mut lines = vec![header];
        lines.extend(content.into_iter().skip(offset).take(body_height));
        let mut lines = lines
            .into_iter()
            .map(|line| format!("{}{line}", " ".repeat(inset)))
            .collect::<Vec<_>>();
        lines.insert(0, rule.clone());
        lines.push(rule);
        lines.truncate(height);
        Self {
            x: 0,
            y: usize::from(placement.anchor_y),
            offset,
            lines,
        }
    }

    /// 【上下文】【浮层绘制】整行清除底层内容后绘制浮层，不铺设背景色。
    /// 参数: 无；返回: 绘制结果。
    fn draw(&self) -> Result<()> {
        let mut output = io::stdout();
        queue!(output, Print("\x1b[?2026h"))?;
        for (row, line) in self.lines.iter().enumerate() {
            let row = (self.y + row) as u16;
            // 1. 先清除整行，避免两侧露出底层会话文字
            queue!(
                output,
                MoveTo(0, row),
                Clear(ClearType::CurrentLine),
                MoveTo(self.x as u16, row),
                Print(format!("{line}\x1b[0m"))
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
    /// 【上下文】【浮层边界测试】极窄、矮屏和普通终端都不越界。
    /// 参数: 无；返回: 无。
    #[test]
    fn overlay_stays_inside_terminal() {
        for (cols, rows) in [(1, 1), (12, 6), (40, 20), (100, 30)] {
            let placement = OverlayPlacement::inline(cols, rows, 2);
            let layout = Layout::new(&ContextOverview::default(), placement, 0);
            assert!(layout.y + layout.lines.len() <= usize::from(rows));
            assert!(layout.lines.iter().all(|line| layout.x
                + crate::render::ansi_clip::ansi_visible_width(line)
                <= usize::from(cols)));
        }
    }
}
