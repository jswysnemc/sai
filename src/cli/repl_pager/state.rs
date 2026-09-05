use super::repl_pager_search::PagerSearch;
use crate::render::transcript::AnsiLine;
use crossterm::event::{KeyCode, KeyModifiers};

/// 展开面板的视图与键盘交互状态。
pub(super) struct PagerState {
    pub(super) index: usize,
    pub(super) scroll: usize,
    pub(super) full: bool,
    pub(super) editing_search: bool,
    pub(super) search: PagerSearch,
    other_scroll: usize,
}

impl PagerState {
    /// 创建分段视图；没有分段时直接展示全文。
    ///
    /// 参数: `start` 为初始分段，`count` 为分段总数
    /// 返回: 初始交互状态
    pub(super) fn new(start: usize, count: usize) -> Self {
        Self {
            index: start.min(count.saturating_sub(1)),
            scroll: 0,
            full: count == 0,
            editing_search: false,
            search: PagerSearch::default(),
            other_scroll: 0,
        }
    }

    /// 处理搜索编辑、视图切换与滚动键。
    ///
    /// 参数: `code` 和 `modifiers` 为按键，`lines` 为当前正文，`height` 为可视行数，`count` 为分段数
    /// 返回: 需要关闭面板时返回 true
    pub(super) fn handle_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
        lines: &[AnsiLine],
        height: usize,
        count: usize,
    ) -> bool {
        let max_scroll = lines.len().saturating_sub(height);
        if modifiers.contains(KeyModifiers::CONTROL) && matches!(code, KeyCode::Char('c' | 'o')) {
            return true;
        }
        // 1. 编辑时所有普通字符都属于搜索词，n/N 只在浏览时导航
        if self.editing_search {
            match code {
                KeyCode::Esc | KeyCode::Enter => self.editing_search = false,
                KeyCode::Backspace => self.search.backspace(lines),
                KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
                    self.search.update("", lines);
                }
                KeyCode::Char(ch)
                    if !modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                {
                    self.search.push_char(ch, lines);
                }
                _ => {}
            }
            self.reveal_match(max_scroll);
            return false;
        }
        // 2. 浏览时切换视图、定位命中或调整滚动位置
        match code {
            KeyCode::Esc | KeyCode::Char('q') => return true,
            KeyCode::Char('/') => self.editing_search = true,
            KeyCode::Char('a') if count > 0 => {
                self.full = !self.full;
                std::mem::swap(&mut self.scroll, &mut self.other_scroll);
            }
            KeyCode::Char('n') if self.search.active() => {
                self.search.next();
                self.reveal_match(max_scroll);
            }
            KeyCode::Char('N') if self.search.active() => {
                self.search.previous();
                self.reveal_match(max_scroll);
            }
            KeyCode::Left | KeyCode::Char('h') if !self.full && count > 1 => {
                self.index = (self.index + count - 1) % count;
                self.scroll = 0;
            }
            KeyCode::Right | KeyCode::Char('l') if !self.full && count > 1 => {
                self.index = (self.index + 1) % count;
                self.scroll = 0;
            }
            KeyCode::Up | KeyCode::Char('k') => self.scroll = self.scroll.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => self.scroll = (self.scroll + 1).min(max_scroll),
            KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(height),
            KeyCode::PageDown | KeyCode::Char(' ') => {
                self.scroll = (self.scroll + height).min(max_scroll);
            }
            KeyCode::Home => self.scroll = 0,
            KeyCode::End => self.scroll = max_scroll,
            _ => {}
        }
        false
    }

    /// 将粘贴文本追加到搜索框；忽略换行和终端控制字符。
    ///
    /// 参数: `text` 为粘贴内容，`lines` 为正文，`height` 为可视行数
    /// 返回: 无
    pub(super) fn paste_search(&mut self, text: &str, lines: &[AnsiLine], height: usize) {
        if !self.editing_search {
            return;
        }
        let mut query = self.search.query.clone();
        query.extend(text.chars().filter(|ch| !ch.is_control()));
        self.search.update(&query, lines);
        self.reveal_match(lines.len().saturating_sub(height));
    }

    /// 在视图重排后重新搜索并定位当前命中。
    ///
    /// 参数: `lines` 为重排后的正文，`height` 为可视行数
    /// 返回: 无
    pub(super) fn content_changed(&mut self, lines: &[AnsiLine], height: usize) {
        self.search.refresh(lines);
        let max_scroll = lines.len().saturating_sub(height);
        self.scroll = self.scroll.min(max_scroll);
        self.reveal_match(max_scroll);
    }

    /// 将当前命中放到视口内。
    ///
    /// 参数: `max_scroll` 为最大滚动位置
    /// 返回: 无
    fn reveal_match(&mut self, max_scroll: usize) {
        if let Some(hit) = self.search.current() {
            self.scroll = hit.min(max_scroll);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 搜索中的 n/N 可以输入，回车确认后才用 n 跳转。
    #[test]
    fn search_accepts_navigation_letters_and_enter_confirms() {
        let lines = vec![
            AnsiLine::new("running".into()),
            AnsiLine::new("running again".into()),
        ];
        let mut state = PagerState::new(0, 2);
        state.handle_key(KeyCode::Char('/'), KeyModifiers::NONE, &lines, 1, 2);
        for ch in "running".chars() {
            state.handle_key(KeyCode::Char(ch), KeyModifiers::NONE, &lines, 1, 2);
        }
        assert_eq!(state.search.query, "running");
        assert_eq!(state.search.current(), Some(0));
        state.handle_key(KeyCode::Enter, KeyModifiers::NONE, &lines, 1, 2);
        assert!(!state.editing_search);
        state.handle_key(KeyCode::Char('n'), KeyModifiers::NONE, &lines, 1, 2);
        assert_eq!(state.scroll, 1);
    }

    /// 两种视图各自保留阅读位置，搜索框输入 a 不触发切换。
    #[test]
    fn toggles_full_view_and_restores_segment_position() {
        let mut state = PagerState::new(2, 3);
        state.scroll = 12;
        state.handle_key(KeyCode::Char('a'), KeyModifiers::NONE, &[], 10, 3);
        assert!(state.full);
        assert_eq!(state.scroll, 0);
        state.scroll = 30;
        state.handle_key(KeyCode::Char('a'), KeyModifiers::NONE, &[], 10, 3);
        assert!(!state.full);
        assert_eq!(state.scroll, 12);
        assert_eq!(state.index, 2);
        state.editing_search = true;
        state.handle_key(KeyCode::Char('a'), KeyModifiers::NONE, &[], 10, 3);
        assert!(!state.full);
        assert_eq!(state.search.query, "a");
    }

    /// 粘贴搜索词和重新折行后，命中位置仍可用于滚动。
    #[test]
    fn pasted_search_reindexes_after_resize() {
        let mut state = PagerState::new(0, 1);
        state.editing_search = true;
        state.paste_search("内容\n", &[AnsiLine::new("内容".into())], 1);
        let lines = vec![AnsiLine::new("标题".into()), AnsiLine::new("内容".into())];
        state.content_changed(&lines, 1);
        assert_eq!(state.search.query, "内容");
        assert_eq!(state.scroll, 1);
        assert!(state
            .search
            .highlight(lines[1].as_str(), 1)
            .contains("\x1b[7m"));
    }
}
