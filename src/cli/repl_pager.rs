use crate::cli::keyboard_enhancement::KeyboardEnhancementState;
use crate::i18n::text as t;
use crate::render::render_expandable_body;
use crate::render::terminal_frame::TerminalFrame;
use crate::render::terminal_rows::paint_changed_rows;
use crate::render::transcript::{AnsiLine, ExpandableBlock, ExpandableBlockKind, PagerView};
use anyhow::Result;
use crossterm::cursor::{Hide, Show};
use crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    Event, KeyEvent, KeyEventKind,
};
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use std::io::{self, Write};
use std::time::Duration;

mod repl_pager_search;
mod scroll;
mod search_highlight;
mod search_index;
mod state;

use scroll::{apply_mouse, horizontal_progress_track, scrollbar_glyphs, ScrollDragTarget};
use state::PagerState;

/// 副屏会话。空闲时阻塞读键；运行中每次只取已经到达的按键，前台轮次继续被 poll。
pub(in crate::cli) struct PagerScreen {
    stdout: io::Stdout,
    was_raw: bool,
    keyboard: KeyboardEnhancementState,
    state: PagerState,
    view: PagerView,
    body_width: usize,
    /// 上次看到的段落数；停在末段时新段落出现就跟着展开
    prev_count: usize,
    drag_target: ScrollDragTarget,
    previous: Option<(usize, usize, Vec<String>)>,
    frame: TerminalFrame,
    active: bool,
}

impl PagerScreen {
    /// 进入备用屏，初始展开 `start` 段。
    ///
    /// 参数: `start` 为初始段落，`count` 为进入时的段落数
    /// 返回: 已接管终端的副屏
    pub(in crate::cli) fn enter(start: usize, count: usize) -> Result<Self> {
        let mut stdout = io::stdout();
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
        let keyboard = if was_raw {
            KeyboardEnhancementState::default()
        } else {
            KeyboardEnhancementState::enable(&mut stdout)
        };
        Ok(Self {
            stdout,
            was_raw,
            keyboard,
            state: PagerState::new(start, count),
            view: PagerView::empty(),
            body_width: 0,
            prev_count: count,
            drag_target: ScrollDragTarget::None,
            previous: None,
            frame: TerminalFrame::new(),
            active: true,
        })
    }

    /// 按当前段落重绘，并处理已经到达的按键。
    ///
    /// 参数: `wait` 为空时阻塞到下一个事件；`Some(0)` 只处理已经在队列里的键。
    /// `render` 用当前焦点和列宽画出和前台同一套的正文
    /// 返回: 用户关闭副屏时为 true
    pub(in crate::cli) fn poll(
        &mut self,
        wait: Option<Duration>,
        mut render: impl FnMut(usize, usize) -> PagerView,
    ) -> Result<bool> {
        let (cols, rows) = terminal::size().unwrap_or((80, 24));
        let rows = rows.max(1) as usize;
        let cols = cols.max(1) as usize;
        let header_rows = usize::from(rows >= 3);
        let footer_rows = if rows >= 4 { 2 } else { usize::from(rows >= 2) };
        let view_h = rows.saturating_sub(header_rows + footer_rows).max(1);
        let body_width = cols.saturating_sub(1).max(1);
        self.layout(&mut render, body_width, view_h);
        let count = self.view.paragraphs;
        self.draw(cols, rows, header_rows, footer_rows, view_h, count)?;
        self.read_input(wait, cols, view_h, header_rows, count)
    }

    /// 离开备用屏并恢复进入前的终端模式。可以重复调用。
    pub(in crate::cli) fn leave(&mut self) -> Result<()> {
        if !self.active {
            return Ok(());
        }
        self.active = false;
        let _ = execute!(self.stdout, DisableMouseCapture);
        if self.was_raw {
            let _ = execute!(self.stdout, Show, LeaveAlternateScreen);
        } else {
            let _ = execute!(self.stdout, DisableBracketedPaste);
            self.keyboard.disable(&mut self.stdout);
            let _ = execute!(self.stdout, Show, LeaveAlternateScreen);
            let _ = terminal::disable_raw_mode();
        }
        let _ = self.stdout.flush();
        Ok(())
    }

    /// 用前台同一套渲染画出正文；搜索命中会改到那一段并重新画。
    fn layout(
        &mut self,
        render: &mut impl FnMut(usize, usize) -> PagerView,
        body_width: usize,
        view_h: usize,
    ) {
        let mut view = render(self.state.index, body_width);
        let count = view.paragraphs;
        if count > 0 {
            self.state.index = self.state.index.min(count - 1);
        }
        if self.prev_count > 0
            && count > self.prev_count
            && self.state.index + 1 == self.prev_count
            && !self.state.editing_search
        {
            self.state.index = count - 1;
            self.state.snap_paragraph = true;
            view = render(self.state.index, body_width);
        }
        self.prev_count = count;
        let width_changed = self.body_width != 0 && body_width != self.body_width;
        self.state
            .content_changed(&view.search_lines, usize::MAX, view_h);
        if width_changed && self.state.search.active() {
            self.state.follow_match = true;
        }
        self.body_width = body_width;
        let follow = std::mem::take(&mut self.state.follow_match);
        let snap = std::mem::take(&mut self.state.snap_paragraph);
        if follow {
            if let Some(hit) = self.state.search.current() {
                if let Some(paragraph) = view.search_paragraph.get(hit).copied().flatten() {
                    let next = paragraph.min(count.saturating_sub(1));
                    // #region agent log
                    debug_agent_log(
                        "E",
                        "repl_pager.rs:layout",
                        "search hit expands paragraph",
                        &format!(
                            "{{\"hit\":{hit},\"paragraph\":{next},\"matches\":{}}}",
                            self.state.search.matches.len()
                        ),
                    );
                    // #endregion
                    if next != self.state.index {
                        self.state.index = next;
                        view = render(self.state.index, body_width);
                    }
                }
            }
        }
        if snap {
            // #region agent log
            debug_agent_log(
                "D",
                "repl_pager.rs:layout",
                "paragraph focus changed",
                &format!("{{\"index\":{},\"count\":{count}}}", self.state.index),
            );
            // #endregion
        }
        self.view = view;
        let max_scroll = self.view.display.len().saturating_sub(view_h);
        if follow {
            if let Some(hit) = self.state.search.current() {
                if let Some(row) = self
                    .view
                    .source
                    .iter()
                    .position(|source| *source == Some(hit))
                {
                    if row < self.state.scroll || row >= self.state.scroll + view_h {
                        self.state.scroll = row.saturating_sub(view_h / 4).min(max_scroll);
                    }
                }
            }
        } else if snap {
            let start = self.view.starts.get(self.state.index).copied().unwrap_or(0);
            self.state.scroll = start.min(max_scroll);
        }
        self.state.scroll = self.state.scroll.min(max_scroll);
    }

    /// 组装标题、展开段、省略行和底栏。
    fn draw(
        &mut self,
        cols: usize,
        rows: usize,
        header_rows: usize,
        footer_rows: usize,
        view_h: usize,
        count: usize,
    ) -> Result<()> {
        let body_width = cols.saturating_sub(1).max(1);
        let shown = if count == 0 { 0 } else { self.state.index + 1 };
        let view_label = format!("{} [{shown}/{count}]", t("Paragraph", "段落"));
        let header_prefix = if self.state.editing_search || self.state.search.active() {
            format!("{view_label}  {}", self.state.search.status_text())
        } else {
            view_label
        };
        let mut page_lines = vec![String::new(); rows];
        if self.previous.is_none() {
            write!(
                self.frame,
                "{}",
                crate::render::terminal_image::KITTY_DELETE_PLACEMENTS
            )?;
        }
        if header_rows > 0 {
            page_lines[0] = format!("\x1b[1m{}\x1b[0m", truncate_visible(&header_prefix, cols));
        }
        let total = self.view.display.len();
        let scrollbar = scrollbar_glyphs(view_h, total, self.state.scroll);
        for row in 0..view_h {
            let idx = self.state.scroll + row;
            let raw = self
                .view
                .display
                .get(idx)
                .map(AnsiLine::as_str)
                .unwrap_or("");
            let line = match self.view.source.get(idx).copied().flatten() {
                Some(source) => self.state.search.highlight(raw, source),
                None => raw.to_string(),
            };
            let bar = scrollbar.get(row).copied().unwrap_or(' ');
            page_lines[header_rows + row] = format!(
                "{}\x1b[2m{}\x1b[0m",
                pad_line(&line, body_width),
                if cols > 1 {
                    bar.to_string()
                } else {
                    String::new()
                }
            );
        }
        let end = (self.state.scroll + view_h).min(total);
        let pct = if total == 0 {
            100
        } else {
            ((end as f64 / total as f64) * 100.0).round() as u16
        };
        if footer_rows > 1 {
            page_lines[header_rows + view_h] =
                horizontal_progress_track(cols, total, view_h, self.state.scroll);
        }
        let hint = if self.state.editing_search {
            t(
                "type to jump · Enter keep · Esc abort · Ctrl+U clear",
                "输入即跳转 · Enter 保留 · Esc 取消 · Ctrl+U 清空",
            )
        } else {
            t(
                "Ctrl+J/K paragraphs · ↑↓ scroll · /? search · n/N jump · Esc close",
                "Ctrl+J/K 段落 · ↑↓ 滚动 · /? 搜索 · n/N 跳转 · Esc 关闭",
            )
        };
        if footer_rows > 0 {
            let footer = format!("{hint}  {pct}%  {shown}/{count}");
            page_lines[rows - 1] = format!("\x1b[2m{}\x1b[0m", truncate_visible(&footer, cols));
        }
        let old_lines = self
            .previous
            .as_ref()
            .filter(|(old_cols, old_rows, _)| *old_cols == cols && *old_rows == rows)
            .map(|(_, _, lines)| lines.as_slice());
        paint_changed_rows(&mut self.frame, 0, cols, &page_lines, old_lines)?;
        self.frame.commit()?;
        self.previous = Some((cols, rows, page_lines));
        Ok(())
    }

    /// 读取一个或一批按键。关闭副屏时返回 true。
    fn read_input(
        &mut self,
        wait: Option<Duration>,
        cols: usize,
        view_h: usize,
        header_rows: usize,
        count: usize,
    ) -> Result<bool> {
        let progress_row = (header_rows + view_h) as u16;
        let max_scroll = self.view.display.len().saturating_sub(view_h);
        let search_lines = self.view.search_lines.clone();
        let display_len = self.view.display.len();
        loop {
            let ready = match wait {
                None => true,
                Some(timeout) => event::poll(timeout)?,
            };
            if !ready {
                return Ok(false);
            }
            match event::read()? {
                Event::Key(KeyEvent {
                    code,
                    modifiers,
                    kind,
                    ..
                }) if kind != KeyEventKind::Release => {
                    self.drag_target = ScrollDragTarget::None;
                    if self.state.handle_key(
                        code,
                        modifiers,
                        &search_lines,
                        display_len,
                        view_h,
                        count,
                    ) {
                        return Ok(true);
                    }
                }
                Event::Paste(text) => self.state.paste_search(&text, &search_lines),
                Event::Mouse(mouse) => {
                    apply_mouse(
                        mouse,
                        cols,
                        view_h,
                        self.view.display.len(),
                        max_scroll,
                        progress_row,
                        header_rows as u16,
                        &mut self.state.scroll,
                        &mut self.drag_target,
                    );
                }
                Event::Resize(_, _) => {
                    self.drag_target = ScrollDragTarget::None;
                    self.previous = None;
                }
                _ => {}
            }
            if wait.is_some() && event::poll(Duration::ZERO)? {
                continue;
            }
            return Ok(false);
        }
    }
}

impl Drop for PagerScreen {
    fn drop(&mut self) {
        let _ = self.leave();
    }
}

/// 在备用屏幕中展示会话：渲染和前台相同，只展开 `start_index` 那一段。
///
/// 参数: `start_index` 为初始段落，`paragraphs` 为段落数，`render` 按焦点和列宽出图
/// 返回: 是否成功
pub(super) fn open_blocks_pager(
    start_index: usize,
    paragraphs: usize,
    mut render: impl FnMut(usize, usize) -> PagerView,
) -> Result<()> {
    let mut screen = PagerScreen::enter(start_index, paragraphs)?;
    let result = (|| -> Result<()> {
        loop {
            if screen.poll(None, &mut render)? {
                break;
            }
        }
        Ok(())
    })();
    let left = screen.leave();
    result.and(left)
}

/// 把调试记录追加到本次会话的日志文件。
pub(in crate::cli) fn debug_agent_log(hypothesis: &str, location: &str, message: &str, data: &str) {
    // #region agent log
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("/home/snemc/workspace/sai/.cursor/debug-ff618c.log")
    else {
        return;
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or(0);
    let _ = writeln!(
        file,
        "{{\"sessionId\":\"ff618c\",\"hypothesisId\":\"{hypothesis}\",\"location\":\"{location}\",\"message\":\"{message}\",\"data\":{data},\"timestamp\":{now}}}"
    );
    // #endregion
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
    open_blocks_pager(0, 1, move |_focus, width| {
        PagerView::from_lines(render_block_lines(&block, width))
    })
}

/// 按实际正文宽度完整渲染一个分段，包括标题和全部内容。
///
/// 参数: `block` 为分段源数据，`width` 为正文列数
/// 返回: 保留样式的完整显示行
pub(super) fn render_block_lines(block: &ExpandableBlock, width: usize) -> Vec<AnsiLine> {
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
