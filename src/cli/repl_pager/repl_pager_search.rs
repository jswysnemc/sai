use super::search_index::{SearchIndex, SourceSpan};
use crate::render::transcript::AnsiLine;

/// Ctrl+O 阅读面板的搜索状态。
#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub(super) struct PagerSearch {
    /// 当前搜索词
    pub(super) query: String,
    /// 命中行在内容行中的下标
    pub(super) matches: Vec<usize>,
    /// 当前高亮的命中序号
    pub(super) selected: usize,
    index: SearchIndex,
    hits: Vec<Vec<SourceSpan>>,
}

impl PagerSearch {
    /// 搜索词是否处于激活状态。
    ///
    /// 返回:
    /// - 搜索词非空时返回 true
    pub(super) fn active(&self) -> bool {
        !self.query.is_empty()
    }

    /// 更新搜索词并重新计算命中。
    ///
    /// 参数:
    /// - `query`: 新搜索词；空值清除搜索
    /// - `content_lines`: 已折行的内容行
    ///
    /// 返回:
    /// - 无
    pub(super) fn update(&mut self, query: &str, content_lines: &[AnsiLine]) {
        let changed = self.index.refresh(content_lines);
        if self.query == query && !changed {
            return;
        }
        self.query = query.to_string();
        self.selected = 0;
        self.hits = self.index.find(query);
        self.matches = self.hits.iter().map(|hit| hit[0].row).collect();
    }

    /// 追加一个字符到搜索词。
    ///
    /// 参数:
    /// - `ch`: 输入字符
    /// - `content_lines`: 已折行的内容行
    ///
    /// 返回:
    /// - 无
    pub(super) fn push_char(&mut self, ch: char, content_lines: &[AnsiLine]) {
        let mut query = self.query.clone();
        query.push(ch);
        self.update(&query, content_lines);
    }

    /// 删除搜索词的最后一个字符。
    ///
    /// 参数:
    /// - `content_lines`: 已折行的内容行
    ///
    /// 返回:
    /// - 无
    pub(super) fn backspace(&mut self, content_lines: &[AnsiLine]) {
        let mut query = self.query.clone();
        query.pop();
        self.update(&query, content_lines);
    }

    /// 重新索引重排后的正文，并保留当前命中序号。
    ///
    /// 参数: `content_lines` 为当前视图内容
    /// 返回: 无
    pub(super) fn refresh(&mut self, content_lines: &[AnsiLine]) {
        let selected = self.selected;
        let query = self.query.clone();
        self.update(&query, content_lines);
        self.selected = selected.min(self.matches.len().saturating_sub(1));
    }

    /// 高亮命中文字，保留 ANSI 颜色和跨行匹配。
    ///
    /// 参数: `line` 为原始样式行，`index` 为内容行下标
    /// 返回: 只在实际命中范围插入高亮样式
    pub(super) fn highlight(&self, line: &str, index: usize) -> String {
        super::search_highlight::highlight(line, index, &self.hits, self.selected)
    }

    /// 跳到下一个命中。
    ///
    /// 返回:
    /// - 命中行下标；无命中时返回空
    pub(super) fn next(&mut self) -> Option<usize> {
        if self.matches.is_empty() {
            return None;
        }
        self.selected = (self.selected + 1) % self.matches.len();
        self.matches.get(self.selected).copied()
    }

    /// 跳到上一个命中。
    ///
    /// 返回:
    /// - 命中行下标；无命中时返回空
    pub(super) fn previous(&mut self) -> Option<usize> {
        if self.matches.is_empty() {
            return None;
        }
        self.selected = self
            .selected
            .checked_sub(1)
            .unwrap_or(self.matches.len() - 1);
        self.matches.get(self.selected).copied()
    }

    /// 当前命中行下标。
    ///
    /// 返回:
    /// - 命中行下标；无命中时返回空
    pub(super) fn current(&self) -> Option<usize> {
        self.matches.get(self.selected).copied()
    }

    /// 渲染搜索状态栏文本。
    ///
    /// 返回:
    /// - `search: query [i/n]` 形式的状态文本
    pub(super) fn status_text(&self) -> String {
        if !self.active() {
            return String::new();
        }
        format!(
            "/{} [{}/{}]",
            self.query,
            if self.matches.is_empty() {
                0
            } else {
                self.selected + 1
            },
            self.matches.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造测试用 ANSI 行。
    fn lines(values: &[&str]) -> Vec<AnsiLine> {
        values
            .iter()
            .map(|value| AnsiLine::new(value.to_string()))
            .collect()
    }

    /// 跨行短语只高亮命中文字，两处同一行命中可独立导航
    #[test]
    fn highlights_exact_cross_line_and_repeated_matches() {
        let content = lines(&["prefix alpha", "  beta suffix alpha beta"]);
        let mut search = PagerSearch::default();
        search.update("alpha beta", &content);
        assert_eq!(search.matches, vec![0, 1]);
        let first = search.highlight(content[0].as_str(), 0);
        assert!(first.starts_with("prefix \x1b[1m\x1b[7malpha"));
        let second = search.highlight(content[1].as_str(), 1);
        assert!(second.contains("beta\x1b[0m suffix \x1b[4malpha beta"));
        search.update("a", &lines(&["a a"]));
        assert_eq!(search.matches, vec![0, 0]);
        assert_eq!(search.next(), Some(0));
        assert_eq!(search.selected, 1);
    }

    /// 颜色片段、中文与大小写展开仍映射到完整原字符，图片载荷不参与搜索
    #[test]
    fn preserves_ansi_images_and_unicode_boundaries() {
        let content = lines(&["\x1b[31m前 İ\x1b[0m中 e\u{301} 后\x1b_Gf=100;hidden\x1b\\"]);
        let mut search = PagerSearch::default();
        search.update("i\u{307}中", &content);
        assert_eq!(search.matches, vec![0]);
        let highlighted = search.highlight(content[0].as_str(), 0);
        assert!(highlighted.starts_with("\x1b[31m前 "));
        assert!(highlighted.contains("\x1b_Gf=100;hidden\x1b\\"));
        search.update("hidden", &content);
        assert!(search.matches.is_empty());
    }

    /// 搜索命中按大小写不敏感统计，导航循环。
    #[test]
    fn search_finds_matches_case_insensitively() {
        let content = lines(&["alpha", "Beta", "gamma alpha", "delta"]);
        let mut search = PagerSearch::default();
        search.update("ALPHA", &content);

        assert_eq!(search.matches, vec![0, 2]);
        assert_eq!(search.current(), Some(0));
        assert_eq!(search.next(), Some(2));
        assert_eq!(search.next(), Some(0));
        assert_eq!(search.previous(), Some(2));
    }

    /// 空搜索词清除命中。
    #[test]
    fn empty_query_clears_matches() {
        let content = lines(&["alpha", "beta"]);
        let mut search = PagerSearch::default();
        search.update("alpha", &content);
        assert_eq!(search.matches.len(), 1);

        search.update("", &content);
        assert!(!search.active());
        assert!(search.matches.is_empty());
        assert_eq!(search.status_text(), "");
    }

    /// 状态栏报告当前序号与总数。
    #[test]
    fn status_text_reports_position() {
        let content = lines(&["one", "two", "one more"]);
        let mut search = PagerSearch::default();
        search.update("one", &content);
        assert_eq!(search.status_text(), "/one [1/2]");

        search.next();
        assert_eq!(search.status_text(), "/one [2/2]");
    }

    /// 搜索使用可见正文，允许关键词跨越语法高亮片段。
    #[test]
    fn search_matches_across_ansi_styles() {
        let content = lines(&["\x1b[31mRu\x1b[0mnn\x1b[1ming\x1b[0m command"]);
        let mut search = PagerSearch::default();
        search.update("running", &content);
        assert_eq!(search.current(), Some(0));
    }

    /// ANSI 颜色参数不能成为用户可见的搜索结果。
    #[test]
    fn search_ignores_terminal_control_parameters() {
        let content = lines(&["\x1b[31mfailed\x1b[0m"]);
        let mut search = PagerSearch::default();
        search.update("31", &content);
        assert!(search.matches.is_empty());
    }
}
