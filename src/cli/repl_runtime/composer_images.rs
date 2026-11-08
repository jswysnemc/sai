use super::ReplRuntime;
use crate::cli::repl_chrome::ReplChrome;
use crate::cli::repl_clipboard::{ReplClipboardBlockSpan, ReplClipboardState};
use anyhow::Result;

impl ReplRuntime {
    /// 【输入图片】【绘制交接】丢弃差异基线前撤下草稿图片，防止缩放或滚动遗留放置。
    /// 参数: 无；返回: 清理指令入帧结果。
    pub(super) fn invalidate_composer_paint(&mut self) -> Result<()> {
        if let Some(signature) = self.last_composer_signature.take() {
            signature.clear_images(&mut self.frame)?;
        }
        Ok(())
    }

    /// 【输入图片】【普通草稿】兼容没有附件的输入更新，避免继承上一个草稿的预览。
    /// 参数: chrome 为样式，input/cursor 为草稿与字符位置，is_pasted 为粘贴标志，clipboard_blocks 为原子块，slash_selection 为候选索引；返回: 顶部行与高度。
    pub(in crate::cli) fn update_composer(
        &mut self,
        chrome: &ReplChrome,
        input: &str,
        cursor: usize,
        is_pasted: bool,
        clipboard_blocks: Vec<ReplClipboardBlockSpan>,
        slash_selection: usize,
    ) -> Result<(u16, u16)> {
        self.update_composer_content(
            chrome,
            input,
            cursor,
            is_pasted,
            clipboard_blocks,
            slash_selection,
            Vec::new(),
        )
    }

    /// 【输入图片】【附件草稿】在计算输入框高度之前同步附件，空闲与流式输入共用同一路径。
    /// 参数: chrome 为样式，input/cursor 为草稿与字符位置，is_pasted 为粘贴标志，clipboard 为附件登记，slash_selection 为候选索引；返回: 顶部行与高度。
    pub(in crate::cli) fn update_composer_with_clipboard(
        &mut self,
        chrome: &ReplChrome,
        input: &str,
        cursor: usize,
        is_pasted: bool,
        clipboard: &ReplClipboardState,
        slash_selection: usize,
    ) -> Result<(u16, u16)> {
        self.update_composer_content(
            chrome,
            input,
            cursor,
            is_pasted,
            clipboard.block_spans(input),
            slash_selection,
            clipboard.composer_image_sources(input),
        )
    }
}
