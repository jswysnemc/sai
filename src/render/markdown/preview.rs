//! 流式未闭合尾行的只读预览，不推进 Markdown 解析状态。

use super::{highlight_code_line, MarkdownStreamRenderer};

impl MarkdownStreamRenderer {
    /// 【终端】【流式预览】组合开放结构与未换行尾部，正文和代码按收到的片段显示。
    /// 参数：无；返回：可替换的 ANSI 预览，不写入稳定内容缓存。
    pub(crate) fn snapshot_live_preview(&mut self) -> String {
        let open = self.snapshot_open_structures();
        if self.buffer.is_empty() {
            return open;
        }
        // 1. 【终端】【流式预览】代码普通行可重复高亮，闭合围栏和图片资产仍等待结构完整
        let renderer = &self.line_renderer;
        if renderer.in_code_block && !renderer.code_is_asset {
            if self.buffer.trim_start().starts_with("```") {
                return open;
            }
            return format!(
                "{}\n",
                highlight_code_line(&renderer.code_lang, &self.buffer)
            );
        }
        // 2. 【终端】【流式预览】普通段落预览复用列表状态；未闭合表格、公式和图片不提前提交
        match self.preview_tail() {
            Some(tail) => {
                let gap = if renderer.pending_blank_lines > 0 && renderer.has_emitted_content {
                    "\n"
                } else {
                    ""
                };
                format!("{open}{gap}{tail}")
            }
            None => open,
        }
    }
}
