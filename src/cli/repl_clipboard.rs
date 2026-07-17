use crate::clipboard::{self, ClipboardChatInput, ClipboardPayload};
use anyhow::Result;

const LONG_TEXT_CHARS: usize = 200;
const LONG_TEXT_LINES: usize = 4;

#[derive(Debug, Clone)]
enum ReplClipboardItem {
    Text { marker: String, text: String },
    Image { marker: String, data_url: String },
}

#[derive(Debug, Default, Clone)]
pub(super) struct ReplClipboardState {
    items: Vec<ReplClipboardItem>,
    next_image_index: usize,
}

impl ReplClipboardState {
    /// 读取系统剪贴板并插入到当前输入。
    ///
    /// 参数:
    /// - `input`: 当前输入内容
    /// - `cursor`: 当前光标字符位置
    ///
    /// 返回:
    /// - 是否作为折叠内容插入
    pub(super) fn paste_into_input(
        &mut self,
        input: &mut String,
        cursor: &mut usize,
    ) -> Result<bool> {
        let payload = clipboard::read_clipboard_payload()?;
        Ok(self.insert_payload(input, cursor, payload))
    }

    /// 清空所有已记录的剪贴板附件。
    pub(super) fn clear(&mut self) {
        self.items.clear();
        self.next_image_index = 0;
    }

    /// 删除光标前方的完整剪贴板占位块。
    ///
    /// 参数:
    /// - `input`: 当前输入内容
    /// - `cursor`: 当前光标字符位置
    ///
    /// 返回:
    /// - 是否删除了占位块
    pub(super) fn remove_block_before_cursor(
        &mut self,
        input: &mut String,
        cursor: &mut usize,
    ) -> bool {
        if let Some((item_index, start, end)) = self.block_range_around_cursor(input, *cursor, true)
        {
            remove_char_range(input, start, end);
            *cursor = start;
            self.items.remove(item_index);
            return true;
        }
        false
    }

    /// 删除光标所在位置的完整剪贴板占位块。
    ///
    /// 参数:
    /// - `input`: 当前输入内容
    /// - `cursor`: 当前光标字符位置
    ///
    /// 返回:
    /// - 是否删除了占位块
    pub(super) fn remove_block_at_cursor(&mut self, input: &mut String, cursor: usize) -> bool {
        if let Some((item_index, start, end)) = self.block_range_around_cursor(input, cursor, false)
        {
            remove_char_range(input, start, end);
            self.items.remove(item_index);
            return true;
        }
        false
    }

    /// 将当前输入和附件组装为聊天输入。
    ///
    /// 参数:
    /// - `input`: 当前输入内容
    ///
    /// 返回:
    /// - 文本消息和可选图片
    pub(super) fn to_chat_input(&self, input: &str) -> ClipboardChatInput {
        let mut message = input.to_string();
        let mut image_url = None::<String>;
        for item in &self.items {
            match item {
                ReplClipboardItem::Text { marker, text } if message.contains(marker) => {
                    message = replace_once(&message, marker, "").trim().to_string();
                    message = clipboard::apply_clipboard_payload(
                        message,
                        ClipboardPayload::Text(text.clone()),
                    )
                    .message;
                }
                ReplClipboardItem::Image { marker, data_url } if message.contains(marker) => {
                    message = replace_once(&message, marker, "").trim().to_string();
                    if image_url.is_none() {
                        image_url = Some(data_url.clone());
                    }
                }
                _ => {}
            }
        }
        if message.trim().is_empty() && image_url.is_some() {
            message = "请根据剪贴板图片回答。".to_string();
        }
        ClipboardChatInput { message, image_url }
    }

    /// 插入指定剪贴板载荷，测试可直接覆盖文本和图片分支。
    ///
    /// 参数:
    /// - `input`: 当前输入内容
    /// - `cursor`: 当前光标字符位置
    /// - `payload`: 剪贴板载荷
    ///
    /// 返回:
    /// - 是否作为折叠内容插入
    fn insert_payload(
        &mut self,
        input: &mut String,
        cursor: &mut usize,
        payload: ClipboardPayload,
    ) -> bool {
        match payload {
            ClipboardPayload::Text(text) => self.insert_text(input, cursor, text),
            ClipboardPayload::ImageDataUrl {
                data_url,
                width,
                height,
            } => {
                self.next_image_index += 1;
                let marker = format!("[image {} {width}x{height}]", self.next_image_index);
                insert_text_at_cursor(input, cursor, &marker);
                self.items
                    .push(ReplClipboardItem::Image { marker, data_url });
                true
            }
        }
    }

    /// 插入剪贴板文本，长文本折叠为占位符。
    ///
    /// 参数:
    /// - `input`: 当前输入内容
    /// - `cursor`: 当前光标字符位置
    /// - `text`: 剪贴板文本
    ///
    /// 返回:
    /// - 是否作为折叠内容插入
    fn insert_text(&mut self, input: &mut String, cursor: &mut usize, text: String) -> bool {
        let trimmed = text.trim().to_string();
        let chars = trimmed.chars().count();
        let lines = trimmed.lines().count();
        if chars <= LONG_TEXT_CHARS && lines <= LONG_TEXT_LINES {
            insert_text_at_cursor(input, cursor, &trimmed);
            return false;
        }
        let marker = format!("[text {chars} chars]");
        insert_text_at_cursor(input, cursor, &marker);
        self.items.push(ReplClipboardItem::Text {
            marker,
            text: trimmed,
        });
        true
    }

    /// 查找光标附近的剪贴板占位块。
    ///
    /// 参数:
    /// - `input`: 当前输入内容
    /// - `cursor`: 当前光标字符位置
    /// - `before`: 是否按 Backspace 语义查找
    ///
    /// 返回:
    /// - 匹配的条目索引、起始字符位置和结束字符位置
    fn block_range_around_cursor(
        &self,
        input: &str,
        cursor: usize,
        before: bool,
    ) -> Option<(usize, usize, usize)> {
        for (item_index, item) in self.items.iter().enumerate() {
            let marker = item.marker();
            for (start_byte, _) in input.match_indices(marker) {
                let start = input[..start_byte].chars().count();
                let end = start + marker.chars().count();
                let matches = if before {
                    cursor > start && cursor <= end
                } else {
                    cursor >= start && cursor < end
                };
                if matches {
                    return Some((item_index, start, end));
                }
            }
        }
        None
    }
}

impl ReplClipboardItem {
    /// 返回剪贴板占位块文本。
    ///
    /// 返回:
    /// - 占位块文本
    fn marker(&self) -> &str {
        match self {
            Self::Text { marker, .. } | Self::Image { marker, .. } => marker,
        }
    }
}

/// 在指定字符位置插入文本。
///
/// 参数:
/// - `value`: 原始字符串
/// - `cursor`: 光标字符位置
/// - `text`: 要插入的文本
fn insert_text_at_cursor(value: &mut String, cursor: &mut usize, text: &str) {
    let byte_index = value
        .char_indices()
        .nth(*cursor)
        .map(|(index, _)| index)
        .unwrap_or(value.len());
    value.insert_str(byte_index, text);
    *cursor += text.chars().count();
}

/// 删除指定字符范围。
///
/// 参数:
/// - `value`: 原始字符串
/// - `start`: 起始字符位置
/// - `end`: 结束字符位置
fn remove_char_range(value: &mut String, start: usize, end: usize) {
    let byte_start = byte_index_for_char(value, start);
    let byte_end = byte_index_for_char(value, end);
    value.replace_range(byte_start..byte_end, "");
}

/// 返回指定字符位置对应的字节位置。
///
/// 参数:
/// - `value`: 原始字符串
/// - `char_index`: 字符位置
///
/// 返回:
/// - 字节位置
fn byte_index_for_char(value: &str, char_index: usize) -> usize {
    value
        .char_indices()
        .nth(char_index)
        .map(|(index, _)| index)
        .unwrap_or(value.len())
}

/// 只替换第一个匹配项。
///
/// 参数:
/// - `value`: 原始字符串
/// - `from`: 要替换的文本
/// - `to`: 替换后的文本
///
/// 返回:
/// - 替换结果
fn replace_once(value: &str, from: &str, to: &str) -> String {
    value.replacen(from, to, 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_text_pastes_inline() {
        let mut state = ReplClipboardState::default();
        let mut input = "问: ".to_string();
        let mut cursor = input.chars().count();

        let folded = state.insert_payload(
            &mut input,
            &mut cursor,
            ClipboardPayload::Text("内容".to_string()),
        );

        assert!(!folded);
        assert_eq!(input, "问: 内容");
        assert_eq!(state.to_chat_input(&input).message, "问: 内容");
    }

    #[test]
    fn long_text_pastes_as_marker_and_submits_full_text() {
        let mut state = ReplClipboardState::default();
        let mut input = "总结 ".to_string();
        let mut cursor = input.chars().count();
        let text = "a".repeat(LONG_TEXT_CHARS + 1);

        let folded = state.insert_payload(
            &mut input,
            &mut cursor,
            ClipboardPayload::Text(text.clone()),
        );
        let chat = state.to_chat_input(&input);

        assert!(folded);
        assert!(input.contains("[text 201 chars]"));
        assert!(chat.message.contains("<clipboard>"));
        assert!(chat.message.contains(&text));
    }

    #[test]
    fn image_pastes_as_marker_and_submits_data_url() {
        let mut state = ReplClipboardState::default();
        let mut input = String::new();
        let mut cursor = 0;

        state.insert_payload(
            &mut input,
            &mut cursor,
            ClipboardPayload::ImageDataUrl {
                data_url: "data:image/png;base64,abc".to_string(),
                width: 800,
                height: 600,
            },
        );
        let chat = state.to_chat_input(&input);

        assert_eq!(input, "[image 1 800x600]");
        assert_eq!(chat.message, "请根据剪贴板图片回答。");
        assert_eq!(chat.image_url.as_deref(), Some("data:image/png;base64,abc"));
    }

    #[test]
    fn backspace_removes_whole_marker() {
        let mut state = ReplClipboardState::default();
        let mut input = String::new();
        let mut cursor = 0;
        state.insert_payload(
            &mut input,
            &mut cursor,
            ClipboardPayload::ImageDataUrl {
                data_url: "data:image/png;base64,abc".to_string(),
                width: 800,
                height: 600,
            },
        );

        assert!(state.remove_block_before_cursor(&mut input, &mut cursor));
        assert!(input.is_empty());
        assert_eq!(cursor, 0);
        assert!(state.to_chat_input(&input).image_url.is_none());
    }

    #[test]
    fn delete_removes_whole_marker() {
        let mut state = ReplClipboardState::default();
        let mut input = "x".to_string();
        let mut cursor = 1;
        state.insert_payload(
            &mut input,
            &mut cursor,
            ClipboardPayload::Text("a".repeat(LONG_TEXT_CHARS + 1)),
        );

        assert!(state.remove_block_at_cursor(&mut input, 1));
        assert_eq!(input, "x");
    }
}
