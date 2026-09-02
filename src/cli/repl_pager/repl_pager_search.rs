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
        self.query = query.to_string();
        self.matches.clear();
        self.selected = 0;
        if !self.active() {
            return;
        }
        let needle = self.query.to_lowercase();
        for (index, line) in content_lines.iter().enumerate() {
            if line.as_str().to_lowercase().contains(&needle) {
                self.matches.push(index);
            }
        }
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
}
