use crate::cli::keyboard_enhancement::KeyboardEnhancementState;
use crate::i18n::text as t;
use crate::render::render_expandable_body;
use crate::render::transcript::{AnsiLine, ExpandableBlock, ExpandableBlockKind};
use anyhow::Result;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyEvent, KeyEventKind,
};
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, queue};
use std::io::{self, Write};

mod repl_pager_search;
mod scroll;
mod state;

use scroll::{apply_mouse, horizontal_progress_track, scrollbar_glyphs, ScrollDragTarget};
use state::PagerState;

/// 在备用屏幕中分页展示可展开块列表，支持左右切换与可拖动进度条。
///
/// 参数:
/// - `blocks`: 全部可展开块（时间序）
/// - `start_index`: 初始展示的块下标
/// - `render_full`: 按正文宽度渲染完整会话的函数
///
/// 返回:
/// - 是否成功
pub(super) fn open_blocks_pager(
    blocks: &[ExpandableBlock],
    start_index: usize,
    mut render_full: impl FnMut(usize) -> Vec<AnsiLine>,
) -> Result<()> {
    let mut stdout = io::stdout();
    // 1. 记录进入前的 raw 状态；pager 结束后恢复，避免打断 TUI 输入
    let was_raw = terminal::is_raw_mode_enabled().unwrap_or(false);
    if !was_raw {
        terminal::enable_raw_mode()?;
    }
    if was_raw {
        execute!(stdout, EnterAlternateScreen, Hide, EnableMouseCapture)?;
    } else {
        execute!(
            stdout,
            EnterAlternateScreen,
            Hide,
            EnableBracketedPaste,
            EnableMouseCapture
        )?;
    }
    let mut keyboard_enhancement = if was_raw {
        KeyboardEnhancementState::default()
    } else {
        KeyboardEnhancementState::enable(&mut stdout)
    };
    let result = (|| -> Result<()> {
        let mut state = PagerState::new(start_index, blocks.len());
        let mut content_key = None;
        let mut content_lines = Vec::new();
        // 拖动目标：横向底栏进度条 / 右侧竖向滚动条
        let mut drag_target = ScrollDragTarget::None;
        loop {
            let (cols, rows) = terminal::size().unwrap_or((80, 24));
            let rows = rows.max(1) as usize;
            let cols = cols.max(1) as usize;
            let header_rows = usize::from(rows >= 3);
            let footer_rows = if rows >= 4 { 2 } else { usize::from(rows >= 2) };
            let view_h = rows.saturating_sub(header_rows + footer_rows).max(1);
            let body_width = cols.saturating_sub(1).max(1);
            // 2. 仅在视图或宽度变化时渲染正文，输入搜索词不重复渲染全部历史
            let key = (state.full, state.index, cols, rows);
            if content_key != Some(key) {
                content_lines = if state.full {
                    render_full(body_width)
                } else {
                    render_block_lines(&blocks[state.index], body_width)
                };
                state.content_changed(&content_lines, view_h);
                content_key = Some(key);
            }
            let max_scroll = content_lines.len().saturating_sub(view_h);
            state.scroll = state.scroll.min(max_scroll);
            let view_label = if state.full {
                t("Full transcript", "全文展开").to_string()
            } else {
                format!(
                    "{} [{}/{}]",
                    t("Segment", "分段展开"),
                    state.index + 1,
                    blocks.len()
                )
            };
            let header_prefix = if state.editing_search || state.search.active() {
                format!(
                    "{view_label}  /{}",
                    state.search.status_text().trim_start_matches('/')
                )
            } else {
                view_label
            };

            queue!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
            // 全屏视图接管前删除 transcript 的图像放置，避免旧图叠在 pager 上
            write!(
                stdout,
                "{}",
                crate::render::terminal_image::KITTY_DELETE_PLACEMENTS
            )?;
            // 4. 固定顶栏：仅块序号
            if header_rows > 0 {
                write!(
                    stdout,
                    "\x1b[1m{}\x1b[0m",
                    truncate_visible(&header_prefix, cols)
                )?;
            }
            let scrollbar = scrollbar_glyphs(view_h, content_lines.len(), state.scroll);
            for row in 0..view_h {
                // 5. 逐行绝对定位，避免极矮终端的最后一行换行触发滚屏
                queue!(stdout, MoveTo(0, (header_rows + row) as u16))?;
                let idx = state.scroll + row;
                let line = content_lines.get(idx).map(AnsiLine::as_str).unwrap_or("");
                let line = state.search.highlight(line, idx);
                let bar = scrollbar.get(row).copied().unwrap_or(' ');
                write!(
                    stdout,
                    "{}\x1b[2m{}\x1b[0m",
                    pad_line(&line, body_width),
                    if cols > 1 {
                        bar.to_string()
                    } else {
                        String::new()
                    }
                )?;
            }
            // 7. 可拖动进度条（第二底栏上方的横向轨道）
            let end = (state.scroll + view_h).min(content_lines.len());
            let pct = if content_lines.is_empty() {
                100
            } else {
                ((end as f64 / content_lines.len() as f64) * 100.0).round() as u16
            };
            if footer_rows > 1 {
                let track =
                    horizontal_progress_track(cols, content_lines.len(), view_h, state.scroll);
                queue!(stdout, MoveTo(0, (header_rows + view_h) as u16))?;
                write!(stdout, "{track}")?;
            }
            // 8. 底栏快捷键
            let base_hint = if state.editing_search {
                t(
                    "Enter accept · Esc back · Ctrl+U clear",
                    "Enter 确认 · Esc 返回 · Ctrl+U 清空",
                )
                .to_string()
            } else {
                t(
                    "a all/segment · / search · n/N next/prev · Esc close",
                    "a 全文/分段 · / 搜索 · n/N 下一处/上一处 · Esc 关闭",
                )
                .to_string()
            };
            let footer = if blocks.len() > 1 && !state.full {
                format!(
                    "{}  {}  {}%  {}/{}",
                    base_hint,
                    t(
                        "←→ blocks · ↑↓/PgUp/PgDn/mouse scroll · drag bar · Esc close",
                        "←→ 切换块 · ↑↓/PgUp/PgDn/鼠标滚动 · 拖动进度条 · Esc 关闭",
                    ),
                    pct,
                    state.index + 1,
                    blocks.len()
                )
            } else {
                format!(
                    "{}  {}  {}%",
                    base_hint,
                    t(
                        "↑↓/PgUp/PgDn/mouse scroll · drag bar · Esc close",
                        "↑↓/PgUp/PgDn/鼠标滚动 · 拖动进度条 · Esc 关闭"
                    ),
                    pct
                )
            };
            if footer_rows > 0 {
                queue!(stdout, MoveTo(0, (rows - 1) as u16))?;
                write!(stdout, "\x1b[2m{}\x1b[0m", truncate_visible(&footer, cols))?;
            }
            stdout.flush()?;

            let progress_row = (header_rows + view_h) as u16;
            match event::read()? {
                Event::Key(KeyEvent {
                    code,
                    modifiers,
                    kind,
                    ..
                }) if kind != KeyEventKind::Release => {
                    drag_target = ScrollDragTarget::None;
                    if state.handle_key(code, modifiers, &content_lines, view_h, blocks.len()) {
                        break;
                    }
                }
                Event::Paste(text) => state.paste_search(&text, &content_lines, view_h),
                Event::Mouse(mouse) => {
                    apply_mouse(
                        mouse,
                        cols,
                        view_h,
                        content_lines.len(),
                        max_scroll,
                        progress_row,
                        header_rows as u16,
                        &mut state.scroll,
                        &mut drag_target,
                    );
                }
                Event::Resize(_, _) => {
                    drag_target = ScrollDragTarget::None;
                }
                _ => {}
            }
        }
        Ok(())
    })();
    // 9. 离开备用屏并恢复进入前的终端输入状态
    let _ = execute!(stdout, DisableMouseCapture);
    if was_raw {
        let _ = execute!(stdout, Show, LeaveAlternateScreen);
    } else {
        let _ = execute!(stdout, DisableBracketedPaste);
        keyboard_enhancement.disable(&mut stdout);
        let _ = execute!(stdout, Show, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
    let _ = stdout.flush();
    result
}

/// 兼容单文本打开（单块）。
///
/// 参数:
/// - `title`: 标题
/// - `body`: 正文
///
/// 返回:
/// - 是否成功
#[allow(dead_code)]
pub(super) fn open_text_pager(title: &str, body: &str) -> Result<()> {
    let block = ExpandableBlock {
        title: title.to_string(),
        body: body.to_string(),
        kind: ExpandableBlockKind::Plain,
    };
    open_blocks_pager(std::slice::from_ref(&block), 0, |width| {
        render_block_lines(&block, width)
    })
}

/// 按实际正文宽度完整渲染一个分段，包括标题和全部内容。
///
/// 参数: `block` 为分段源数据，`width` 为正文列数
/// 返回: 保留样式的完整显示行
fn render_block_lines(block: &ExpandableBlock, width: usize) -> Vec<AnsiLine> {
    crate::render::render_expand::with_expanded_render(|| {
        crate::render::render_width::with_render_width(width, || {
            let body = render_expandable_body(block.kind, &block.body);
            let mut lines = AnsiLine::wrap_block(&block.title, width);
            if !block.title.trim().is_empty() && !body.trim().is_empty() {
                lines.push(AnsiLine::new(String::new()));
            }
            lines.extend(AnsiLine::wrap_block(&body, width));
            lines
        })
    })
}

/// 将单行正文填充到固定可见宽度，避免滚动条错位。
///
/// 参数:
/// - `line`: 可能含 ANSI 的正文行
/// - `width`: 目标可见宽度
///
/// 返回:
/// - 右侧补空格后的文本
fn pad_line(line: &str, width: usize) -> String {
    let visible = visible_width_ansi(line);
    if visible >= width {
        return line.to_string();
    }
    format!("{line}{}", " ".repeat(width - visible))
}

/// 计算含 ANSI 文本的可见宽度。
///
/// 参数:
/// - `value`: 原文
///
/// 返回:
/// - 可见列数
fn visible_width_ansi(value: &str) -> usize {
    let plain = crate::render::activity_animation::strip_ansi_for_test(value);
    unicode_width::UnicodeWidthStr::width(plain.as_str())
}

/// 按可见显示宽度截断文本。
///
/// 参数:
/// - `value`: 原文
/// - `width`: 最大列宽
///
/// 返回:
/// - 截断文本
fn truncate_visible(value: &str, width: usize) -> String {
    crate::render::clip_to_width(value, width, "")
}
