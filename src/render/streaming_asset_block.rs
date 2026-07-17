use crate::render::streaming_replace::{clear_rendered_rows, raw_visual_rows};

/// Markdown 资产块流式替换状态。
pub(crate) struct StreamingAssetBlock {
    raw_visual_rows: usize,
    mode: AssetPreviewMode,
}

/// 图片资产在不同输出表面中的预览策略。
#[derive(Clone, Copy)]
enum AssetPreviewMode {
    ReplaceTerminalRows,
    SourcePreview,
    StableFinal,
}

impl StreamingAssetBlock {
    /// 创建资产块流式替换状态。
    ///
    /// 返回:
    /// - 新的资产块替换状态
    pub(crate) fn new() -> Self {
        Self {
            raw_visual_rows: 0,
            mode: AssetPreviewMode::ReplaceTerminalRows,
        }
    }

    /// 创建 source-backed 实时预览状态。
    ///
    /// 返回:
    /// - 仅展示原始 Markdown 的资产状态
    pub(crate) fn new_source_preview() -> Self {
        Self {
            raw_visual_rows: 0,
            mode: AssetPreviewMode::SourcePreview,
        }
    }

    /// 创建 source-backed 定稿状态。
    ///
    /// 返回:
    /// - 仅展示最终图片且不包含光标回退序列的资产状态
    pub(crate) fn new_stable() -> Self {
        Self {
            raw_visual_rows: 0,
            mode: AssetPreviewMode::StableFinal,
        }
    }

    /// 推入一行资产块原始文本。
    ///
    /// 参数:
    /// - `line`: 当前收到的 Markdown 原始行
    ///
    /// 返回:
    /// - 需要立即写入终端的原始 Markdown 行
    pub(crate) fn push_line(&mut self, line: &str) -> String {
        self.raw_visual_rows += raw_visual_rows(line);
        match self.mode {
            AssetPreviewMode::ReplaceTerminalRows | AssetPreviewMode::SourcePreview => {
                format!("{line}\n")
            }
            AssetPreviewMode::StableFinal => String::new(),
        }
    }

    /// 结束资产块并用最终渲染结果替换原始文本。
    ///
    /// 参数:
    /// - `rendered`: 图片渲染文本或错误提示
    ///
    /// 返回:
    /// - 清除原文后的最终渲染文本
    pub(crate) fn finish(&mut self, rendered: String) -> String {
        let output = match self.mode {
            AssetPreviewMode::ReplaceTerminalRows => {
                let mut output = clear_rendered_rows(self.raw_visual_rows);
                output.push_str(&rendered);
                output
            }
            AssetPreviewMode::SourcePreview => String::new(),
            AssetPreviewMode::StableFinal => rendered,
        };
        self.raw_visual_rows = 0;
        output
    }

    /// 重置资产块替换状态。
    pub(crate) fn reset(&mut self) {
        self.raw_visual_rows = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_streamed_asset_source_with_rendered_output() {
        let mut block = StreamingAssetBlock::new();

        assert_eq!(block.push_line("```mermaid"), "```mermaid\n");
        assert_eq!(block.push_line("graph TD"), "graph TD\n");
        let output = block.finish("[diagram]\n".to_string());

        assert!(output.starts_with("\x1b[1A\r\x1b[2K"));
        assert!(output.ends_with("[diagram]\n"));
    }

    #[test]
    fn stable_asset_emits_only_final_image_payload() {
        let mut block = StreamingAssetBlock::new_stable();

        assert!(block.push_line("```mermaid").is_empty());
        assert!(block.push_line("graph TD").is_empty());
        let output = block.finish("[diagram]\n".to_string());

        assert_eq!(output, "[diagram]\n");
        assert!(!output.contains("\x1b[1A"));
    }

    #[test]
    fn source_preview_keeps_raw_asset_until_finalization() {
        let mut block = StreamingAssetBlock::new_source_preview();

        assert_eq!(block.push_line("```mermaid"), "```mermaid\n");
        assert_eq!(block.push_line("graph TD"), "graph TD\n");
        assert!(block.finish("[diagram]\n".to_string()).is_empty());
    }
}
