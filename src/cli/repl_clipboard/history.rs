use super::*;
use crate::state::input_history::{
    InputHistoryAttachment, InputHistoryAttachmentKind, InputHistoryEntry,
};

impl ReplClipboardState {
    /// 【终端】【历史输入】复制仍在输入中的原子块，保留完整正文与图片数据。
    /// 参数: `input` 为含原子标签的输入文字
    /// 返回: 不依赖当前剪贴板生命周期的输入快照
    pub(in crate::cli) fn history_entry(&self, input: &str) -> InputHistoryEntry {
        let attachments = self
            .item_ranges(input)
            .into_iter()
            .map(|(_, item)| {
                let content = match item {
                    ReplClipboardItem::Text { text, .. }
                    | ReplClipboardItem::Reference { text, .. } => text,
                    ReplClipboardItem::Image { data_url, .. } => data_url,
                };
                let kind = match item.kind() {
                    ReplClipboardBlockKind::Text => InputHistoryAttachmentKind::Text,
                    ReplClipboardBlockKind::Image => InputHistoryAttachmentKind::Image,
                    ReplClipboardBlockKind::File => InputHistoryAttachmentKind::File,
                    ReplClipboardBlockKind::Skill => InputHistoryAttachmentKind::Skill,
                };
                InputHistoryAttachment {
                    marker: item.marker().into(),
                    content: content.clone(),
                    kind,
                }
            })
            .collect();
        InputHistoryEntry {
            text: input.into(),
            attachments,
        }
    }

    /// 【终端】【历史输入】恢复附件登记和标签计数，随后粘贴不会覆盖旧附件。
    /// 参数: `entry` 为持久化输入快照
    /// 返回: 可独立编辑的剪贴板状态
    pub(in crate::cli) fn from_history_entry(entry: &InputHistoryEntry) -> Self {
        let mut state = Self::default();
        for attachment in &entry.attachments {
            if attachment.marker.is_empty() || !entry.text.contains(&attachment.marker) {
                continue;
            }
            let marker = attachment.marker.clone();
            let text = attachment.content.clone();
            let item = match attachment.kind {
                InputHistoryAttachmentKind::Text => {
                    state.next_text_index = state.next_text_index.max(marker_index(&marker));
                    ReplClipboardItem::Text { marker, text }
                }
                InputHistoryAttachmentKind::Image => {
                    state.next_image_index = state.next_image_index.max(marker_index(&marker));
                    ReplClipboardItem::Image {
                        marker,
                        data_url: text,
                    }
                }
                InputHistoryAttachmentKind::File => ReplClipboardItem::Reference {
                    marker,
                    text,
                    kind: ReplClipboardBlockKind::File,
                },
                InputHistoryAttachmentKind::Skill => ReplClipboardItem::Reference {
                    marker,
                    text,
                    kind: ReplClipboardBlockKind::Skill,
                },
            };
            state.items.push(item);
        }
        state
    }
}

/// 【终端】【历史输入】读取已有文本或图片标签的序号。
/// 参数: `marker` 为原子标签
/// 返回: 有效序号，旧标签没有序号时为零
fn marker_index(marker: &str) -> usize {
    marker
        .split_whitespace()
        .nth(1)
        .and_then(|index| index.parse().ok())
        .unwrap_or(0)
}
