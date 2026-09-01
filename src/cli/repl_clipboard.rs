use crate::clipboard::{self, ClipboardChatInput, ClipboardPayload};
use crate::config::PasteImageKey;
use anyhow::Result;
use base64::Engine as _;
use crossterm::event::{KeyCode, KeyModifiers};

/// 判断这一按键是否命中配置的「读取剪贴板」键位。
///
/// 两个输入框（空闲态与流式态）共用这一份判定，避免两边各自写键位条件后
/// 慢慢走偏。
///
/// 参数:
/// - `key`: 配置的键位
/// - `code`: 终端按键码
/// - `modifiers`: 终端修饰键
///
/// 返回:
/// - 命中时为 true
pub(super) fn is_paste_key(key: PasteImageKey, code: KeyCode, modifiers: KeyModifiers) -> bool {
    if code != KeyCode::Char('v') {
        return false;
    }
    let ctrl = modifiers.contains(KeyModifiers::CONTROL);
    let alt = modifiers.contains(KeyModifiers::ALT);
    match key {
        PasteImageKey::CtrlV => ctrl,
        PasteImageKey::AltV => alt,
        PasteImageKey::Both => ctrl || alt,
    }
}

/// 剪贴板里有图片时按图片插入，供 `Event::Paste` 分支先探测。
///
/// Windows 终端把 Ctrl+V 转成括号粘贴的文本事件，图片不会以文本形式到达，
/// 只能反过来查系统剪贴板。其他平台的括号粘贴事件带着真文本，探测只会
/// 白读一次剪贴板，因此只在 Windows 上启用。
///
/// 参数:
/// - `state`: 剪贴板状态
/// - `input`: 当前输入内容
/// - `cursor`: 当前光标字符位置
///
/// 返回:
/// - 剪贴板包含图片且已插入时返回 true
pub(super) fn paste_image_first(
    state: &mut ReplClipboardState,
    input: &mut String,
    cursor: &mut usize,
) -> bool {
    #[cfg(windows)]
    {
        state.paste_image_if_any(input, cursor)
    }
    #[cfg(not(windows))]
    {
        let _ = (state, input, cursor);
        false
    }
}

const LONG_TEXT_CHARS: usize = 200;
const LONG_TEXT_LINES: usize = 4;
/// 单行超过该字符数时也折叠为占位块（避免单行巨长文本撑爆输入区）
const LONG_LINE_CHARS: usize = 160;

#[derive(Debug, Clone)]
enum ReplClipboardItem {
    Text { marker: String, text: String },
    Image { marker: String, data_url: String },
}

/// 输入区中剪贴板原子块的种类。
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) enum ReplClipboardBlockKind {
    Text,
    Image,
}

/// 剪贴板原子块在输入字符串中的字符区间。
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(super) struct ReplClipboardBlockSpan {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) kind: ReplClipboardBlockKind,
}

#[derive(Debug, Default, Clone)]
pub(super) struct ReplClipboardState {
    items: Vec<ReplClipboardItem>,
    next_text_index: usize,
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
        self.next_text_index = 0;
        self.next_image_index = 0;
    }

    /// Windows 终端把 Ctrl+V 转成括号粘贴的文本事件，图片不会以文本形式到达。
    /// 剪贴板里有图片时按图片插入，供 `Event::Paste` 分支先探测。
    ///
    /// 参数:
    /// - `input`: 当前输入内容
    /// - `cursor`: 当前光标字符位置
    ///
    /// 返回:
    /// - 剪贴板包含图片且已插入时返回 true
    #[cfg(windows)]
    pub(super) fn paste_image_if_any(&mut self, input: &mut String, cursor: &mut usize) -> bool {
        match clipboard::read_clipboard_payload() {
            Ok(payload @ ClipboardPayload::ImageDataUrl { .. }) => {
                self.insert_payload(input, cursor, payload);
                true
            }
            _ => false,
        }
    }

    /// 将括号粘贴事件中的文本插入输入区，长文本会生成原子块。
    ///
    /// 参数:
    /// - `input`: 当前输入内容
    /// - `cursor`: 当前光标字符位置
    /// - `text`: 粘贴文本
    ///
    /// 返回:
    /// - 是否生成了折叠原子块
    pub(super) fn paste_text_into_input(
        &mut self,
        input: &mut String,
        cursor: &mut usize,
        text: String,
    ) -> bool {
        self.insert_text(input, cursor, text)
    }

    /// 用一次系统剪贴板粘贴替换已经被 Windows 终端逐字写入的首行。
    ///
    /// 参数:
    /// - `input`: 当前输入内容
    /// - `cursor`: 当前光标字符位置
    /// - `prefix_chars`: 光标前需要替换的字符数
    /// - `text`: 系统剪贴板的完整正文
    ///
    /// 返回:
    /// - 是否生成了折叠原子块
    pub(super) fn replace_recent_text_with_paste(
        &mut self,
        input: &mut String,
        cursor: &mut usize,
        prefix_chars: usize,
        text: String,
    ) -> bool {
        if prefix_chars == 0 || prefix_chars > *cursor {
            return false;
        }
        let start = cursor.saturating_sub(prefix_chars);
        remove_char_range(input, start, *cursor);
        *cursor = start;
        self.insert_text(input, cursor, text)
    }

    /// 返回当前输入中的剪贴板原子块区间。
    ///
    /// 参数:
    /// - `input`: 当前输入内容
    ///
    /// 返回:
    /// - 按输入顺序排列的原子块区间
    pub(super) fn block_spans(&self, input: &str) -> Vec<ReplClipboardBlockSpan> {
        let mut spans = Vec::new();
        for item in &self.items {
            let marker = item.marker();
            let kind = item.kind();
            if let Some(start_byte) = input.find(marker) {
                let start = input[..start_byte].chars().count();
                spans.push(ReplClipboardBlockSpan {
                    start,
                    end: start + marker.chars().count(),
                    kind,
                });
            }
        }
        spans.sort_by_key(|span| span.start);
        spans
    }

    /// 把输入中的长文本占位块替换回完整正文，并按顺序摘出图片占位块。
    ///
    /// 供外部编辑器使用：折叠的长文本进编辑器后要能直接读写，
    /// 图片则是 base64 data URL，塞进编辑器只会刷屏，必须先摘出去。
    ///
    /// 参数:
    /// - `input`: 当前输入内容
    ///
    /// 返回:
    /// - `(展开长文本并去掉图片后的正文, 按出现顺序排列的 (锚点字符位置, 图片占位符))`
    ///   锚点以返回正文的字符位置计量
    pub(super) fn expand_for_editor(&self, input: &str) -> (String, Vec<(usize, String)>) {
        let mut text = input.to_string();
        // 1. 长文本占位块替换为完整正文，编辑器里可直接修改
        for item in &self.items {
            if let ReplClipboardItem::Text { marker, text: body } = item {
                if text.contains(marker.as_str()) {
                    text = replace_once(&text, marker, body);
                }
            }
        }
        // 2. 图片占位块摘出，记录其在剩余正文中的字符位置
        let mut images = Vec::new();
        for item in &self.items {
            let ReplClipboardItem::Image { marker, .. } = item else {
                continue;
            };
            let Some(start_byte) = text.find(marker.as_str()) else {
                continue;
            };
            let anchor = text[..start_byte].chars().count();
            text = replace_once(&text, marker, "");
            images.push((anchor, marker.clone()));
        }
        images.sort_by_key(|(anchor, _)| *anchor);
        (text, images)
    }

    /// 判断输入中是否存在折叠的长文本占位块。
    ///
    /// 参数:
    /// - `input`: 当前输入内容
    ///
    /// 返回:
    /// - 存在长文本占位块时为 true
    pub(super) fn has_text_blocks(&self, input: &str) -> bool {
        self.items.iter().any(|item| {
            matches!(item, ReplClipboardItem::Text { .. }) && input.contains(item.marker())
        })
    }

    /// 生成发送后的用户回显文本：仅当输入含粘贴长文本占位块时展开正文并标记需折叠。
    ///
    /// 普通键入长消息保持原文、不折叠；图片占位块保留字面标记。
    ///
    /// 参数:
    /// - `input`: 当前输入（可含 `[text N … chars]` 占位块）
    ///
    /// 返回:
    /// - `(回显正文, 是否按思考块语义折叠)`
    pub(super) fn echo_text_for_submit(&self, input: &str) -> (String, bool) {
        let fold = self.has_text_blocks(input);
        if !fold {
            return (input.to_string(), false);
        }
        let mut text = input.to_string();
        for item in &self.items {
            if let ReplClipboardItem::Text { marker, text: body } = item {
                if text.contains(marker.as_str()) {
                    text = replace_once(&text, marker, body);
                }
            }
        }
        (text, true)
    }

    /// 丢弃全部长文本占位块的登记，图片登记保持不变。
    ///
    /// 外部编辑器已把长文本展开成正文，占位块不再对应输入中的任何片段；
    /// 若继续保留，提交时会按 marker 二次展开，正文出现重复。
    pub(super) fn forget_text_blocks(&mut self) {
        self.items
            .retain(|item| !matches!(item, ReplClipboardItem::Text { .. }));
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

    /// 计算光标左移一格后的位置（原子块整体跳过）。
    ///
    /// 参数:
    /// - `input`: 当前输入内容
    /// - `cursor`: 当前光标字符位置
    ///
    /// 返回:
    /// - 新光标位置
    pub(super) fn cursor_left(&self, input: &str, cursor: usize) -> usize {
        let target = cursor.saturating_sub(1);
        for span in self.block_spans(input) {
            // 落点进入块内部时直接跳到块首，保持占位块的原子性
            if target > span.start && target < span.end {
                return span.start;
            }
        }
        target
    }

    /// 计算光标右移一格后的位置（原子块整体跳过）。
    ///
    /// 参数:
    /// - `input`: 当前输入内容
    /// - `cursor`: 当前光标字符位置
    ///
    /// 返回:
    /// - 新光标位置
    pub(super) fn cursor_right(&self, input: &str, cursor: usize) -> usize {
        let total = input.chars().count();
        let target = cursor.saturating_add(1).min(total);
        for span in self.block_spans(input) {
            // 落点进入块内部时直接跳到块尾，保持占位块的原子性
            if target > span.start && target < span.end {
                return span.end;
            }
        }
        target
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

    /// 把另一终端转发过来的图片挂进剪贴板状态。
    ///
    /// 转发只带 data URL，尺寸得自己从 PNG 头里读出来：占位块文本带着
    /// 宽高，写死成 0x0 会让回显里出现 `[image 1 0x0]` 这种假尺寸。
    ///
    /// 参数:
    /// - `input`: 当前输入内容
    /// - `cursor`: 当前光标字符位置
    /// - `data_url`: 图片的 PNG data URL
    ///
    /// 返回:
    /// - 成功插入时返回 true
    pub(super) fn insert_image_data_url(
        &mut self,
        input: &mut String,
        cursor: &mut usize,
        data_url: String,
    ) -> bool {
        let Some((width, height)) = png_data_url_size(&data_url) else {
            return false;
        };
        self.insert_payload(
            input,
            cursor,
            ClipboardPayload::ImageDataUrl {
                data_url,
                width,
                height,
            },
        )
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
        let lines = trimmed
            .lines()
            .count()
            .max(if trimmed.is_empty() { 0 } else { 1 });
        // 1. 任一行超长也按折叠处理，不再只看总行数
        let max_line_chars = trimmed
            .lines()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(chars);
        if chars <= LONG_TEXT_CHARS && lines <= LONG_TEXT_LINES && max_line_chars <= LONG_LINE_CHARS
        {
            insert_text_at_cursor(input, cursor, &trimmed);
            return false;
        }
        self.next_text_index += 1;
        let marker = format!("[text {} {chars} chars]", self.next_text_index);
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

    /// 返回原子块的渲染类型。
    fn kind(&self) -> ReplClipboardBlockKind {
        match self {
            Self::Text { .. } => ReplClipboardBlockKind::Text,
            Self::Image { .. } => ReplClipboardBlockKind::Image,
        }
    }
}

/// 读出 PNG data URL 的像素尺寸。
///
/// 只读 PNG 头的 IHDR：转发过来的图片只需要宽高来拼占位块文本，
/// 整图解一次码纯属浪费。
///
/// 参数:
/// - `data_url`: 形如 `data:image/png;base64,…` 的 data URL
///
/// 返回:
/// - 读取成功时的（宽, 高）
fn png_data_url_size(data_url: &str) -> Option<(usize, usize)> {
    let encoded = data_url.split_once("base64,")?.1;
    // 宽高是 IHDR 的头两个字段，落在第 16 字节起；取前 24 字节足够
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .ok()?;
    let ihdr = bytes.get(16..24)?;
    let width = u32::from_be_bytes(ihdr[0..4].try_into().ok()?);
    let height = u32::from_be_bytes(ihdr[4..8].try_into().ok()?);
    Some((width as usize, height as usize))
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

    /// 三档配置 × 常见按键：只认自己那一档，且不误吞词级移动的 Alt+←/→。
    #[test]
    fn is_paste_key_matches_only_the_configured_key() {
        let v = KeyCode::Char('v');
        let left = KeyCode::Left;
        let ctrl = KeyModifiers::CONTROL;
        let alt = KeyModifiers::ALT;
        let none = KeyModifiers::NONE;

        assert!(is_paste_key(PasteImageKey::CtrlV, v, ctrl));
        assert!(!is_paste_key(PasteImageKey::CtrlV, v, alt));
        assert!(!is_paste_key(PasteImageKey::CtrlV, v, none));

        assert!(is_paste_key(PasteImageKey::AltV, v, alt));
        assert!(!is_paste_key(PasteImageKey::AltV, v, ctrl));
        assert!(!is_paste_key(PasteImageKey::AltV, v, none));

        assert!(is_paste_key(PasteImageKey::Both, v, ctrl));
        assert!(is_paste_key(PasteImageKey::Both, v, alt));
        assert!(!is_paste_key(PasteImageKey::Both, v, none));

        // 词级移动与别的字符都不该被当成粘贴
        for key in [PasteImageKey::CtrlV, PasteImageKey::AltV, PasteImageKey::Both] {
            assert!(!is_paste_key(key, left, ctrl));
            assert!(!is_paste_key(key, left, alt));
            assert!(!is_paste_key(key, KeyCode::Char('c'), ctrl));
            assert!(!is_paste_key(key, KeyCode::Enter, none));
        }
    }

    /// 非 Windows 上括号粘贴带着真文本，探测只会白读一次剪贴板。
    #[test]
    #[cfg(not(windows))]
    fn paste_image_first_is_a_noop_off_windows() {
        let mut state = ReplClipboardState::default();
        let mut input = String::new();
        let mut cursor = 0;
        assert!(!paste_image_first(&mut state, &mut input, &mut cursor));
        assert!(input.is_empty());
    }

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
    fn single_long_line_pastes_as_marker() {
        let mut state = ReplClipboardState::default();
        let mut input = String::new();
        let mut cursor = 0usize;
        // 字符总数未超 LONG_TEXT_CHARS，但单行超 LONG_LINE_CHARS
        let text = "x".repeat(LONG_LINE_CHARS + 1);
        let folded = state.insert_payload(
            &mut input,
            &mut cursor,
            ClipboardPayload::Text(text.clone()),
        );
        assert!(folded);
        assert!(input.starts_with("[text 1 "));
        let chat = state.to_chat_input(&input);
        assert_eq!(chat.message, text);
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
        assert!(input.contains("[text 1 201 chars]"));
        assert!(chat.message.contains("<clipboard>"));
        assert!(chat.message.contains(&text));
    }

    #[test]
    fn windows_key_stream_replaces_typed_first_line_with_one_marker() {
        let mut state = ReplClipboardState::default();
        let mut input = "前缀第一行".to_string();
        let mut cursor = input.chars().count();
        let text = format!("第一行\n{}", "第二行\n".repeat(80));

        let folded = state.replace_recent_text_with_paste(
            &mut input,
            &mut cursor,
            "第一行".chars().count(),
            text.clone(),
        );
        let chat = state.to_chat_input(&input);

        assert!(folded);
        assert!(input.starts_with("前缀[text 1 "));
        assert_eq!(input.matches("[text ").count(), 1);
        assert!(chat.message.contains(text.trim()));
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

    #[test]
    fn cursor_moves_skip_whole_block() {
        let mut state = ReplClipboardState::default();
        let mut input = "x".to_string();
        let mut cursor = 1;
        state.insert_payload(
            &mut input,
            &mut cursor,
            ClipboardPayload::Text("a".repeat(LONG_TEXT_CHARS + 1)),
        );
        let span = state.block_spans(&input)[0];
        assert_eq!(cursor, span.end);

        // 1. 从块尾左移：整体跳到块首
        assert_eq!(state.cursor_left(&input, span.end), span.start);
        // 2. 从块首右移：整体跳到块尾
        assert_eq!(state.cursor_right(&input, span.start), span.end);
        // 3. 块外移动仍逐字符
        assert_eq!(state.cursor_left(&input, span.start), span.start - 1);
        assert_eq!(
            state.cursor_right(&input, span.end),
            span.end.min(input.chars().count())
        );
    }

    #[test]
    fn block_spans_identify_text_and_image_markers() {
        let mut state = ReplClipboardState::default();
        let mut input = String::new();
        let mut cursor = 0;
        state.insert_payload(
            &mut input,
            &mut cursor,
            ClipboardPayload::Text("a".repeat(LONG_TEXT_CHARS + 1)),
        );
        state.insert_payload(
            &mut input,
            &mut cursor,
            ClipboardPayload::ImageDataUrl {
                data_url: "data:image/png;base64,abc".to_string(),
                width: 10,
                height: 20,
            },
        );

        let spans = state.block_spans(&input);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].kind, ReplClipboardBlockKind::Text);
        assert_eq!(spans[1].kind, ReplClipboardBlockKind::Image);
        assert_eq!(spans[0].end, spans[1].start);
    }

    #[test]
    fn echo_text_for_submit_expands_only_pasted_text_blocks() {
        let mut state = ReplClipboardState::default();
        let mut input = String::from("前缀 ");
        let mut cursor = input.chars().count();
        let pasted = "a".repeat(LONG_TEXT_CHARS + 1);
        state.paste_text_into_input(&mut input, &mut cursor, pasted.clone());

        let (echo, fold) = state.echo_text_for_submit(&input);
        assert!(fold);
        assert!(echo.starts_with("前缀 "));
        assert!(echo.contains(&pasted));
        assert!(!echo.contains("[text "));

        let typed = "x\n".repeat(20);
        let (plain_echo, plain_fold) = ReplClipboardState::default().echo_text_for_submit(&typed);
        assert!(!plain_fold);
        assert_eq!(plain_echo, typed);
    }

    #[test]
    fn text_block_uses_distinct_color_without_changing_width() {
        let mut state = ReplClipboardState::default();
        let mut input = String::new();
        let mut cursor = 0;
        state.paste_text_into_input(&mut input, &mut cursor, "a".repeat(LONG_TEXT_CHARS + 1));

        let styled = crate::cli::repl_input_render::style_clipboard_line(
            &input,
            0,
            &state.block_spans(&input),
        );
        assert!(styled.contains("\x1b[48;5;25m"));
        assert_eq!(
            crate::cli::repl_text::visible_width(&styled),
            input.chars().count()
        );
    }
}
