use super::repl_pager_search::PagerSearch;
use crate::render::transcript::AnsiLine;
use crossterm::event::{KeyCode, KeyModifiers};

/// 展开面板的视图与键盘交互状态。
pub(super) struct PagerState {
    pub(super) index: usize,
    pub(super) scroll: usize,
    pub(super) editing_search: bool,
    pub(super) search: PagerSearch,
    /// `/` 向前，`?` 向后；`n` 沿这个方向跳，`N` 反向跳
    search_forward: bool,
    /// 进入搜索前的词、段落和滚动位置，Esc 取消时恢复
    checkpoint: Option<(String, usize, usize)>,
    /// 搜索词或 n/N 变化后，画面应按命中段落展开并滚到该行
    pub(super) follow_match: bool,
    /// Ctrl+J/K 切换段落后，画面应滚到该段开头
    pub(super) snap_paragraph: bool,
}

impl PagerState {
    /// 创建副屏状态，初始展开 `start` 段。
    ///
    /// 参数: `start` 为初始段落，`count` 为段落总数
    /// 返回: 初始交互状态
    pub(super) fn new(start: usize, count: usize) -> Self {
        Self {
            index: start.min(count.saturating_sub(1)),
            scroll: 0,
            editing_search: false,
            search: PagerSearch::default(),
            search_forward: true,
            checkpoint: None,
            follow_match: false,
            snap_paragraph: false,
        }
    }

    /// 处理搜索、段落切换与滚动。
    ///
    /// 参数: `search_lines` 为全部段落的完整正文，`display_len` 为当前屏幕行数，
    /// `height` 为可视行数，`count` 为段落数
    /// 返回: 需要关闭面板时返回 true
    pub(super) fn handle_key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
        search_lines: &[AnsiLine],
        display_len: usize,
        height: usize,
        count: usize,
    ) -> bool {
        let max_scroll = display_len.saturating_sub(height);
        if modifiers.contains(KeyModifiers::CONTROL) && matches!(code, KeyCode::Char('c' | 'o')) {
            return true;
        }
        // 1. 搜索编辑：普通字符属于搜索词；Esc 放弃并回到进入搜索前的位置
        if self.editing_search {
            match code {
                KeyCode::Esc => self.abort_search(search_lines),
                KeyCode::Enter => {
                    self.editing_search = false;
                    self.checkpoint = None;
                    self.follow_match = self.search.active();
                }
                KeyCode::Backspace => {
                    self.search.backspace(search_lines);
                    self.follow_match = true;
                }
                KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
                    self.search.update("", search_lines);
                    self.follow_match = true;
                }
                KeyCode::Char(ch)
                    if !modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                {
                    self.search.push_char(ch, search_lines);
                    self.follow_match = true;
                }
                _ => {}
            }
            return false;
        }
        // 2. 浏览：Ctrl+J/K 换段，/ 与 ? 搜索，n/N 沿搜索方向跳转
        match code {
            KeyCode::Esc | KeyCode::Char('q') => return true,
            KeyCode::Char('/') | KeyCode::Char('?') => {
                self.checkpoint = Some((self.search.query.clone(), self.index, self.scroll));
                self.search_forward = matches!(code, KeyCode::Char('/'));
                self.search.set_forward(self.search_forward);
                self.search.update("", search_lines);
                self.editing_search = true;
            }
            KeyCode::Char('n') if self.search.active() => {
                self.search.step(self.search_forward);
                self.follow_match = true;
            }
            KeyCode::Char('N') if self.search.active() => {
                self.search.step(!self.search_forward);
                self.follow_match = true;
            }
            KeyCode::Char('j') if modifiers.contains(KeyModifiers::CONTROL) && count > 1 => {
                self.index = (self.index + 1) % count;
                self.snap_paragraph = true;
            }
            KeyCode::Char('k') if modifiers.contains(KeyModifiers::CONTROL) && count > 1 => {
                self.index = (self.index + count - 1) % count;
                self.snap_paragraph = true;
            }
            KeyCode::Up | KeyCode::Char('k') => self.scroll = self.scroll.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => self.scroll = (self.scroll + 1).min(max_scroll),
            KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(height),
            KeyCode::PageDown | KeyCode::Char(' ') => {
                self.scroll = (self.scroll + height).min(max_scroll);
            }
            KeyCode::Home | KeyCode::Char('g') => self.scroll = 0,
            KeyCode::End | KeyCode::Char('G') => self.scroll = max_scroll,
            _ => {}
        }
        false
    }

    /// 取消尚未确认的搜索，恢复进入 `/` 或 `?` 之前的位置。
    fn abort_search(&mut self, search_lines: &[AnsiLine]) {
        self.editing_search = false;
        self.follow_match = false;
        if let Some((query, index, scroll)) = self.checkpoint.take() {
            self.search.set_forward(true);
            self.search.update(&query, search_lines);
            self.index = index;
            self.scroll = scroll;
        }
    }

    /// 将粘贴文本追加到搜索框；忽略换行和终端控制字符。
    ///
    /// 参数: `text` 为粘贴内容，`lines` 为全部段落的完整正文
    /// 返回: 无
    pub(super) fn paste_search(&mut self, text: &str, lines: &[AnsiLine]) {
        if !self.editing_search {
            return;
        }
        let mut query = self.search.query.clone();
        query.extend(text.chars().filter(|ch| !ch.is_control()));
        self.search.update(&query, lines);
        self.follow_match = true;
    }

    /// 正文折行变化后重新搜索，滚动位置按新的屏幕高度夹紧。
    ///
    /// 参数: `lines` 为重排后的完整正文，`display_len` 为当前屏幕行数，`height` 为可视行数
    /// 返回: 无
    pub(super) fn content_changed(
        &mut self,
        lines: &[AnsiLine],
        display_len: usize,
        height: usize,
    ) {
        self.search.refresh(lines);
        let max_scroll = display_len.saturating_sub(height);
        self.scroll = self.scroll.min(max_scroll);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(
        state: &mut PagerState,
        code: KeyCode,
        modifiers: KeyModifiers,
        lines: &[AnsiLine],
        display_len: usize,
        height: usize,
        count: usize,
    ) -> bool {
        state.handle_key(code, modifiers, lines, display_len, height, count)
    }

    /// 搜索中的 n 可以输入，回车确认后才用 n 跳到下一处。
    #[test]
    fn search_accepts_navigation_letters_and_enter_confirms() {
        let lines = vec![
            AnsiLine::new("running".into()),
            AnsiLine::new("running again".into()),
        ];
        let mut state = PagerState::new(0, 2);
        key(
            &mut state,
            KeyCode::Char('/'),
            KeyModifiers::NONE,
            &lines,
            lines.len(),
            1,
            2,
        );
        for ch in "running".chars() {
            key(
                &mut state,
                KeyCode::Char(ch),
                KeyModifiers::NONE,
                &lines,
                lines.len(),
                1,
                2,
            );
        }
        assert_eq!(state.search.query, "running");
        assert_eq!(state.search.current(), Some(0));
        key(
            &mut state,
            KeyCode::Enter,
            KeyModifiers::NONE,
            &lines,
            lines.len(),
            1,
            2,
        );
        assert!(!state.editing_search);
        key(
            &mut state,
            KeyCode::Char('n'),
            KeyModifiers::NONE,
            &lines,
            lines.len(),
            1,
            2,
        );
        assert_eq!(state.search.current(), Some(1));
        assert!(state.follow_match);
    }

    /// Ctrl+J/K 在段落之间循环，普通 j/k 只滚动。
    #[test]
    fn ctrl_j_and_k_switch_paragraphs() {
        let mut state = PagerState::new(0, 3);
        key(
            &mut state,
            KeyCode::Char('j'),
            KeyModifiers::CONTROL,
            &[],
            10,
            5,
            3,
        );
        assert_eq!(state.index, 1);
        assert!(state.snap_paragraph);
        state.snap_paragraph = false;
        key(
            &mut state,
            KeyCode::Char('k'),
            KeyModifiers::CONTROL,
            &[],
            10,
            5,
            3,
        );
        assert_eq!(state.index, 0);
        key(
            &mut state,
            KeyCode::Char('j'),
            KeyModifiers::NONE,
            &[],
            10,
            5,
            3,
        );
        assert_eq!(state.index, 0);
        assert_eq!(state.scroll, 1);
    }

    /// Esc 取消尚未确认的搜索，回到进入搜索前的段落和滚动位置。
    #[test]
    fn escape_aborts_search_and_restores_position() {
        let lines = vec![
            AnsiLine::new("alpha".into()),
            AnsiLine::new("beta needle".into()),
        ];
        let mut state = PagerState::new(0, 2);
        state.scroll = 4;
        key(
            &mut state,
            KeyCode::Char('/'),
            KeyModifiers::NONE,
            &lines,
            8,
            3,
            2,
        );
        for ch in "needle".chars() {
            key(
                &mut state,
                KeyCode::Char(ch),
                KeyModifiers::NONE,
                &lines,
                8,
                3,
                2,
            );
        }
        key(
            &mut state,
            KeyCode::Esc,
            KeyModifiers::NONE,
            &lines,
            8,
            3,
            2,
        );
        assert!(!state.editing_search);
        assert!(state.search.query.is_empty());
        assert_eq!(state.index, 0);
        assert_eq!(state.scroll, 4);
    }

    /// `?` 从最后一处命中开始。
    #[test]
    fn backward_search_starts_at_the_last_hit() {
        let lines = vec![
            AnsiLine::new("hit".into()),
            AnsiLine::new("other".into()),
            AnsiLine::new("hit again".into()),
        ];
        let mut state = PagerState::new(0, 1);
        key(
            &mut state,
            KeyCode::Char('?'),
            KeyModifiers::NONE,
            &lines,
            lines.len(),
            2,
            1,
        );
        for ch in "hit".chars() {
            key(
                &mut state,
                KeyCode::Char(ch),
                KeyModifiers::NONE,
                &lines,
                lines.len(),
                2,
                1,
            );
        }
        assert_eq!(state.search.current(), Some(2));
    }

    /// 粘贴搜索词和重新折行后，命中位置仍可用于高亮。
    #[test]
    fn pasted_search_reindexes_after_resize() {
        let mut state = PagerState::new(0, 1);
        state.editing_search = true;
        state.paste_search("内容\n", &[AnsiLine::new("内容".into())]);
        let lines = vec![AnsiLine::new("标题".into()), AnsiLine::new("内容".into())];
        state.content_changed(&lines, lines.len(), 1);
        assert_eq!(state.search.query, "内容");
        assert_eq!(state.search.current(), Some(1));
        assert!(state
            .search
            .highlight(lines[1].as_str(), 1)
            .contains("\x1b[7m"));
    }
}
