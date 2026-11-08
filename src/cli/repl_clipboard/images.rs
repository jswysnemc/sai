use super::{ReplClipboardItem, ReplClipboardState};

impl ReplClipboardState {
    /// 【输入图片】【草稿来源】按草稿占位符顺序返回真实图片，忽略删除项与普通标签。
    /// 参数: input 为当前草稿；返回: 图片数据 URL 列表，不改变附件提交顺序。
    pub(in crate::cli) fn composer_image_sources(&self, input: &str) -> Vec<String> {
        self.item_ranges(input)
            .into_iter()
            .filter_map(|(_, item)| match item {
                ReplClipboardItem::Image { data_url, .. } => Some(data_url.clone()),
                _ => None,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipboard::ClipboardPayload;

    /// 【输入图片】【来源回归】顺序跟随草稿，删除与伪造标签不会残留预览。
    /// 参数: 无；返回: 无。
    #[test]
    fn composer_sources_follow_registered_markers_and_deletion() {
        let mut state = ReplClipboardState::default();
        let mut input = String::new();
        let mut cursor = 0;
        for source in ["first", "second"] {
            state.insert_payload(
                &mut input,
                &mut cursor,
                ClipboardPayload::ImageDataUrl {
                    data_url: source.into(),
                    width: 10,
                    height: 20,
                },
            );
        }
        let reordered = "[image 2 10x20] 中文 [image 1 10x20] [image 99 10x20]";
        assert_eq!(state.composer_image_sources(reordered), ["second", "first"]);
        assert!(state.remove_block_before_cursor(&mut input, &mut cursor));
        assert_eq!(state.composer_image_sources(&input), ["first"]);
        assert!(state.composer_image_sources("[image 99 10x20]").is_empty());
        let restored = ReplClipboardState::from_history_entry(&state.history_entry(&input));
        assert_eq!(restored.composer_image_sources(&input), ["first"]);
    }
}
