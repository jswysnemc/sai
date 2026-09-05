use super::*;
use crate::render::input_atom::{InputAtom, InputEcho};
use std::ops::Range;

impl ReplClipboardState {
    /// 查找仍存在于输入中的已登记原子块，重复引用按出现顺序各占一个区间。
    ///
    /// 参数: `input` 为包含占位符的输入
    /// 返回: 按 UTF-8 字节位置排序的原子块与登记项
    pub(super) fn item_ranges<'a>(
        &'a self,
        input: &str,
    ) -> Vec<(Range<usize>, &'a ReplClipboardItem)> {
        let mut ranges: Vec<(Range<usize>, &ReplClipboardItem)> = Vec::new();
        for item in &self.items {
            if let Some((start, marker)) = input
                .match_indices(item.marker())
                .find(|(start, _)| !ranges.iter().any(|(range, _)| range.contains(start)))
            {
                ranges.push((start..start + marker.len(), item));
            }
        }
        ranges.sort_by_key(|(range, _)| range.start);
        ranges
    }

    /// 【终端】【输入回显】展开提交正文并保留每个原子块的类型与位置。
    ///
    /// 参数: `input` 为输入框内容
    /// 返回: 完整正文及真实原子块元数据，普通方括号文本不生成元数据
    pub(in crate::cli) fn echo_text_for_submit(&self, input: &str) -> InputEcho {
        let mut echo = InputEcho::default();
        let mut cursor = 0;
        for (range, item) in self.item_ranges(input) {
            // 1. 根据原始输入扫描，避免粘贴正文中的同名占位符被二次展开
            echo.text.push_str(&input[cursor..range.start]);
            let start = echo.text.len();
            let body = match item {
                ReplClipboardItem::Text { text, .. }
                | ReplClipboardItem::Reference { text, .. } => text,
                ReplClipboardItem::Image { marker, .. } => marker,
            };
            echo.text.push_str(body);
            // 2. 范围始终对应展开后的正文，中文和混排附件使用相同路径
            echo.atoms.push(InputAtom {
                range: start..echo.text.len(),
                label: item.marker().to_string(),
                kind: item.kind(),
            });
            cursor = range.end;
        }
        echo.text.push_str(&input[cursor..]);
        echo
    }

    /// 将输入与已登记附件组装为模型输入，引用恢复为原始路径或技能名称。
    ///
    /// 参数: `input` 为输入框内容
    /// 返回: 文本消息及可选图片
    pub(in crate::cli) fn to_chat_input(&self, input: &str) -> ClipboardChatInput {
        let mut message = String::new();
        let mut image_url = None;
        let mut pasted = Vec::new();
        let mut cursor = 0;
        for (range, item) in self.item_ranges(input) {
            message.push_str(&input[cursor..range.start]);
            match item {
                ReplClipboardItem::Text { text, .. } => pasted.push(text.clone()),
                ReplClipboardItem::Image { data_url, .. } => {
                    image_url.get_or_insert_with(|| data_url.clone());
                }
                ReplClipboardItem::Reference { text, .. } => message.push_str(text),
            }
            cursor = range.end;
        }
        message.push_str(&input[cursor..]);
        message = message.trim().to_string();
        for text in pasted {
            message =
                clipboard::apply_clipboard_payload(message, ClipboardPayload::Text(text)).message;
        }
        if message.is_empty() && image_url.is_some() {
            message = "请根据剪贴板图片回答。".to_string();
        }
        ClipboardChatInput { message, image_url }
    }
}
