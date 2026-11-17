//! 配置列表通用的 `/` 关键字过滤：统一管理输入法启停、真实光标移动、粘贴与两段式 Esc 清空。

use crate::i18n::text as t;
use crossterm::event::KeyCode;

/// 状态栏内用于标定真实硬件光标列位置的零宽控制标记（绘制前剥离，不写入终端）。
pub(crate) const SEARCH_CURSOR_MARK: char = '\x1f';

/// 列表过滤框状态机。
#[derive(Debug, Default)]
pub(crate) struct ListSearch {
    /// 当前过滤词；空串表示不过滤
    pub query: String,
    /// 当前插入光标所在的字符下标
    pub cursor: usize,
    /// 是否正处于输入过滤词状态
    pub editing: bool,
}

/// 一次按键或粘贴对搜索框产生的影响。
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum SearchEffect {
    /// 搜索框未消费该事件，交由外层列表处理
    Passthrough,
    /// 过滤词发生变化，外层列表应重算命中并重置选中下标
    Updated,
    /// 搜索态切换、光标移动或过滤词被清空，外层仅需重绘而不将按键视为退出
    Closed,
}

impl Drop for ListSearch {
    /// 【配置搜索】【输入法恢复】离开包含搜索框的页面时确保关闭编辑态输入法。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    fn drop(&mut self) {
        if self.editing {
            super::ime::disable_for_nav();
        }
    }
}

impl ListSearch {
    /// 【配置搜索】【按键处理】分发进入搜索、编辑过滤词与两段式 Esc 清空逻辑。
    ///
    /// 参数:
    /// - `key`: 终端按键码
    ///
    /// 返回:
    /// - 搜索框对该按键的消费结果
    pub(crate) fn handle(&mut self, key: KeyCode) -> SearchEffect {
        if self.editing {
            return self.handle_editing(key);
        }
        // 1. 【配置搜索】【两段退出】保留过滤词时首次按 Esc 仅清空过滤词，第二次按 Esc 才退出页面
        if !self.query.is_empty() && matches!(key, KeyCode::Esc) {
            self.query.clear();
            self.cursor = 0;
            return SearchEffect::Closed;
        }
        // 2. 【配置搜索】【开启输入】按 `/` 清空旧词、置于行首并打开系统输入法支持中文过滤
        if matches!(key, KeyCode::Char('/')) {
            self.editing = true;
            self.query.clear();
            self.cursor = 0;
            super::ime::enable_for_edit();
            return SearchEffect::Updated;
        }
        SearchEffect::Passthrough
    }

    /// 【配置搜索】【粘贴处理】在搜索编辑态下将粘贴文本净化后插入当前光标位置。
    ///
    /// 参数:
    /// - `text`: 终端粘贴事件原始文本
    ///
    /// 返回:
    /// - 搜索框对粘贴内容的处理结果
    pub(crate) fn handle_paste(&mut self, text: &str) -> SearchEffect {
        if !self.editing {
            return SearchEffect::Passthrough;
        }
        let cleaned = super::input::sanitize_paste_single_line(text);
        if cleaned.is_empty() {
            return SearchEffect::Closed;
        }
        insert_str_at_char(&mut self.query, &mut self.cursor, &cleaned);
        SearchEffect::Updated
    }

    /// 【配置搜索】【文本匹配】按大小写不敏感子串判断列表行是否命中当前过滤词。
    ///
    /// 参数:
    /// - `text`: 待匹配的列表项文本
    ///
    /// 返回:
    /// - 空过滤词或包含子串时返回 true
    pub(crate) fn matches(&self, text: &str) -> bool {
        let query = self.query.trim();
        query.is_empty() || text.to_lowercase().contains(&query.to_lowercase())
    }

    /// 【配置搜索】【帮助条生成】生成搜索态或保留过滤态下的底栏说明，编辑态内嵌真实光标定位标记。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 搜索激活或过滤非空时返回底栏文本，否则返回 None
    pub(crate) fn help(&self) -> Option<String> {
        if !self.editing && self.query.is_empty() {
            return None;
        }
        let query = if self.editing {
            if self.query.is_empty() {
                format!("{SEARCH_CURSOR_MARK}{}", t("type to filter", "输入以过滤"))
            } else {
                let (before, after) = split_at_char(&self.query, self.cursor);
                format!("{before}{SEARCH_CURSOR_MARK}{after}")
            }
        } else {
            self.query.clone()
        };
        Some(super::theme::help_line(&[
            ("/", &query),
            ("↑↓", t("move", "移动")),
            ("Enter", t("keep filter", "保留过滤")),
            ("Esc", t("clear", "清除")),
        ]))
    }

    /// 【配置搜索】【编辑态按键】处理光标左右移动、删除、确认保留及取消清空。
    ///
    /// 参数:
    /// - `key`: 终端按键码
    ///
    /// 返回:
    /// - 编辑态按键响应结果
    fn handle_editing(&mut self, key: KeyCode) -> SearchEffect {
        let len = self.query.chars().count();
        self.cursor = self.cursor.min(len);
        match key {
            KeyCode::Esc => {
                self.editing = false;
                self.query.clear();
                self.cursor = 0;
                super::ime::disable_for_nav();
                SearchEffect::Closed
            }
            KeyCode::Enter => {
                self.editing = false;
                super::ime::disable_for_nav();
                SearchEffect::Closed
            }
            KeyCode::Left => {
                self.cursor = self.cursor.saturating_sub(1);
                SearchEffect::Closed
            }
            KeyCode::Right => {
                self.cursor = (self.cursor + 1).min(len);
                SearchEffect::Closed
            }
            KeyCode::Home => {
                self.cursor = 0;
                SearchEffect::Closed
            }
            KeyCode::End => {
                self.cursor = len;
                SearchEffect::Closed
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    remove_char_before(&mut self.query, &mut self.cursor);
                    if self.query.is_empty() {
                        self.editing = false;
                        super::ime::disable_for_nav();
                        SearchEffect::Closed
                    } else {
                        SearchEffect::Updated
                    }
                } else if self.query.is_empty() {
                    self.editing = false;
                    super::ime::disable_for_nav();
                    SearchEffect::Closed
                } else {
                    SearchEffect::Closed
                }
            }
            KeyCode::Delete => {
                if self.cursor < len {
                    remove_char_at(&mut self.query, self.cursor);
                    SearchEffect::Updated
                } else {
                    SearchEffect::Closed
                }
            }
            KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown => {
                SearchEffect::Passthrough
            }
            KeyCode::Char(ch) => {
                insert_char_at(&mut self.query, &mut self.cursor, ch);
                SearchEffect::Updated
            }
            _ => SearchEffect::Passthrough,
        }
    }
}

/// 【配置搜索】【字符切分】按字符下标将字符串拆分为光标前后两段。
///
/// 参数:
/// - `value`: 原始文本
/// - `char_index`: 字符游标位置
///
/// 返回:
/// - `(光标前切片, 光标后切片)`
pub(crate) fn split_at_char(value: &str, char_index: usize) -> (&str, &str) {
    let byte_idx = byte_index_for_char(value, char_index);
    value.split_at(byte_idx)
}

/// 【配置搜索】【字符插入】在字符游标处插入单个字符并前移游标。
///
/// 参数:
/// - `value`: 目标字符串
/// - `cursor`: 字符游标位置
/// - `ch`: 待插入字符
///
/// 返回:
/// - 无
pub(crate) fn insert_char_at(value: &mut String, cursor: &mut usize, ch: char) {
    let byte_idx = byte_index_for_char(value, *cursor);
    value.insert(byte_idx, ch);
    *cursor += 1;
}

/// 【配置搜索】【文本插入】在字符游标处插入字符串片段并同步推进游标。
///
/// 参数:
/// - `value`: 目标字符串
/// - `cursor`: 字符游标位置
/// - `text`: 待插入文本
///
/// 返回:
/// - 无
pub(crate) fn insert_str_at_char(value: &mut String, cursor: &mut usize, text: &str) {
    let byte_idx = byte_index_for_char(value, *cursor);
    value.insert_str(byte_idx, text);
    *cursor += text.chars().count();
}

/// 【配置搜索】【前向删除】删除字符游标前的一个字符并回退游标。
///
/// 参数:
/// - `value`: 目标字符串
/// - `cursor`: 字符游标位置
///
/// 返回:
/// - 无
pub(crate) fn remove_char_before(value: &mut String, cursor: &mut usize) {
    if *cursor == 0 {
        return;
    }
    let end = byte_index_for_char(value, *cursor);
    let start = byte_index_for_char(value, *cursor - 1);
    value.replace_range(start..end, "");
    *cursor -= 1;
}

/// 【配置搜索】【当前位删除】删除字符游标所在位置的字符。
///
/// 参数:
/// - `value`: 目标字符串
/// - `cursor`: 字符游标位置
///
/// 返回:
/// - 无
pub(crate) fn remove_char_at(value: &mut String, cursor: usize) {
    if cursor >= value.chars().count() {
        return;
    }
    let start = byte_index_for_char(value, cursor);
    let end = byte_index_for_char(value, cursor + 1);
    value.replace_range(start..end, "");
}

/// 【配置搜索】【字节定位】将字符下标换算为 UTF-8 字节偏移。
///
/// 参数:
/// - `value`: 字符串引用
/// - `char_index`: 字符序号
///
/// 返回:
/// - 对应的 UTF-8 字节偏移量
fn byte_index_for_char(value: &str, char_index: usize) -> usize {
    value
        .char_indices()
        .nth(char_index)
        .map(|(index, _)| index)
        .unwrap_or(value.len())
}

/// 【配置搜索】【下标钳制】将选中下标限制在列表合法范围内，空列表固定返回 0。
///
/// 参数:
/// - `index`: 期望下标
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

    /// 【配置搜索】【新建查询测试】验证按 `/` 开启编辑并支持大小写不敏感匹配。
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

    /// 【配置搜索】【两段退出测试】验证保留过滤词后首次 Esc 清空过滤，第二次 Esc 放行退出。
    #[test]
    fn escape_clears_before_it_can_leave_the_page() {
        let mut search = ListSearch {
            query: "gpt".to_string(),
            cursor: 3,
            editing: false,
        };
        assert_eq!(search.handle(KeyCode::Esc), SearchEffect::Closed);
        assert!(search.query.is_empty());
        assert_eq!(search.handle(KeyCode::Esc), SearchEffect::Passthrough);
    }

    /// 【配置搜索】【游标与粘贴测试】验证编辑态支持光标移动插入、粘贴以及光标标记输出。
    #[test]
    fn cursor_movement_and_paste_update_query_at_cursor() {
        let mut search = ListSearch::default();
        search.handle(KeyCode::Char('/'));
        assert_eq!(search.handle_paste("mini\n"), SearchEffect::Updated);
        assert_eq!(search.query, "mini");
        assert_eq!(search.cursor, 4);

        search.handle(KeyCode::Home);
        search.handle(KeyCode::Char('4'));
        assert_eq!(search.query, "4mini");
        assert_eq!(search.cursor, 1);

        let help = search.help().unwrap();
        assert!(help.contains(&format!("4{SEARCH_CURSOR_MARK}mini")));
    }

    /// 【配置搜索】【空列表边界测试】验证空列表钳制不下溢。
    #[test]
    fn empty_list_index_does_not_underflow() {
        assert_eq!(clamp_index(4, 0), 0);
        assert_eq!(clamp_index(4, 2), 1);
        assert_eq!(clamp_index(0, 3), 0);
    }
}
