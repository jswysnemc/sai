//! 配置列表的 `/` 搜索。
//!
//! 长列表（模型、Skill、Agent、知识库）共用同一套按键：`/` 开始，
//! 输入即时过滤，Enter 保留过滤并回到移动，Esc 清除后才退出页面。

use crate::i18n::text as t;
use crossterm::event::KeyCode;

/// 列表过滤框。
#[derive(Debug, Default)]
pub(crate) struct ListSearch {
    /// 当前过滤词；空表示不过滤
    pub query: String,
    /// 是否正在输入过滤词
    pub editing: bool,
}

/// 一次按键对搜索框的影响。
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum SearchEffect {
    /// 搜索框没有吃掉这个键，交给列表自己处理
    Passthrough,
    /// 过滤词变了，调用方应跳到第一条命中
    Updated,
    /// 搜索结束或被清除，调用方不要再把这个键当成退出
    Closed,
}

impl ListSearch {
    /// 处理一个按键。
    ///
    /// 参数:
    /// - `key`: 终端按键
    ///
    /// 返回:
    /// - 搜索框是否消费了该键
    pub(crate) fn handle(&mut self, key: KeyCode) -> SearchEffect {
        if self.editing {
            return self.handle_editing(key);
        }
        if !self.query.is_empty() && matches!(key, KeyCode::Esc) {
            self.query.clear();
            return SearchEffect::Closed;
        }
        if matches!(key, KeyCode::Char('/')) {
            self.editing = true;
            self.query.clear();
            return SearchEffect::Updated;
        }
        SearchEffect::Passthrough
    }

    /// 过滤词是否命中一段可见文本（大小写不敏感）。
    ///
    /// 参数:
    /// - `text`: 列表行文本
    ///
    /// 返回:
    /// - 空过滤词视为全部命中
    pub(crate) fn matches(&self, text: &str) -> bool {
        let query = self.query.trim();
        query.is_empty() || text.to_lowercase().contains(&query.to_lowercase())
    }

    /// 搜索态的底栏说明。
    ///
    /// 返回:
    /// - 正在输入或已有过滤词时的帮助；否则为空，调用方用自己的帮助条
    pub(crate) fn help(&self) -> Option<String> {
        if !self.editing && self.query.is_empty() {
            return None;
        }
        let query = if self.query.is_empty() {
            t("type to filter", "输入以过滤")
        } else {
            self.query.as_str()
        };
        Some(super::theme::help_line(&[
            ("/", query),
            ("↑↓", t("move", "移动")),
            ("Enter", t("keep filter", "保留过滤")),
            ("Esc", t("clear", "清除")),
        ]))
    }

    fn handle_editing(&mut self, key: KeyCode) -> SearchEffect {
        match key {
            KeyCode::Esc => {
                self.editing = false;
                self.query.clear();
                SearchEffect::Closed
            }
            KeyCode::Enter => {
                self.editing = false;
                SearchEffect::Closed
            }
            KeyCode::Backspace => {
                self.query.pop();
                if self.query.is_empty() {
                    self.editing = false;
                    SearchEffect::Closed
                } else {
                    SearchEffect::Updated
                }
            }
            KeyCode::Up
            | KeyCode::Down
            | KeyCode::Home
            | KeyCode::End
            | KeyCode::PageUp
            | KeyCode::PageDown => SearchEffect::Passthrough,
            KeyCode::Char(ch) => {
                self.query.push(ch);
                SearchEffect::Updated
            }
            _ => SearchEffect::Passthrough,
        }
    }
}

/// 把选中下标夹进列表。空列表固定为 0，避免 `len - 1` 下溢成最大值后高亮消失。
///
/// 参数:
/// - `index`: 当前下标
/// - `len`: 列表长度
///
/// 返回:
/// - 合法下标
pub(crate) fn clamp_index(index: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else {
        index.min(len - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slash_starts_a_fresh_query() {
        let mut search = ListSearch::default();
        assert_eq!(search.handle(KeyCode::Char('/')), SearchEffect::Updated);
        assert!(search.editing);
        assert!(search.query.is_empty());
        assert_eq!(search.handle(KeyCode::Char('g')), SearchEffect::Updated);
        assert_eq!(search.query, "g");
        assert!(search.matches("GPT-4o"));
        assert!(!search.matches("claude"));
    }

    #[test]
    fn escape_clears_before_it_can_leave_the_page() {
        let mut search = ListSearch {
            query: "gpt".to_string(),
            editing: false,
        };
        assert_eq!(search.handle(KeyCode::Esc), SearchEffect::Closed);
        assert!(search.query.is_empty());
        assert_eq!(search.handle(KeyCode::Esc), SearchEffect::Passthrough);
    }

    #[test]
    fn empty_list_index_does_not_underflow() {
        assert_eq!(clamp_index(4, 0), 0);
        assert_eq!(clamp_index(4, 2), 1);
        assert_eq!(clamp_index(0, 3), 0);
    }
}
