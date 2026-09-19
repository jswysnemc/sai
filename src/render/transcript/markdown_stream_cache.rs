use crate::render::markdown::MarkdownStreamRenderer;
use crate::render::table::live_layout::TableLayouts;

/// 【终端】【增量解析】保留已闭合内容和解析状态，开放结构仅生成临时快照
#[derive(Default)]
pub(super) struct MarkdownStreamCache {
    source: String,
    stable: String,
    preview: String,
    renderer: Option<MarkdownStreamRenderer>,
    geometry: (usize, usize),
    #[cfg(test)]
    pub(super) parsed_bytes: usize,
}

impl MarkdownStreamCache {
    /// 只解析新增源码，并在尺寸或既有内容变化时重建缓存
    /// 参数: source 为完整源码，width 为列数，layouts 为固定布局，budget 为可变行数
    /// 返回: 已闭合正文与开放结构快照组成的 ANSI 文本
    pub(super) fn render(
        &mut self,
        source: &str,
        width: usize,
        layouts: &mut TableLayouts,
        budget: usize,
    ) -> String {
        // 1. 【终端】【增量解析】宽度改变或非追加编辑时废弃旧解析状态
        if self.renderer.is_none()
            || self.geometry != (width, budget)
            || !source.starts_with(&self.source)
        {
            self.source.clear();
            self.stable.clear();
            self.preview.clear();
            let mut renderer = MarkdownStreamRenderer::new_source_preview();
            renderer.set_table_layouts(layouts.clone(), Some(budget));
            self.renderer = Some(renderer);
            self.geometry = (width, budget);
        }
        // 2. 【终端】【增量解析】相同源码直接复用；公式和图片仍由原渲染器处理
        if source.len() != self.source.len() {
            let delta = &source[self.source.len()..];
            let renderer = self.renderer.as_mut().expect("initialized renderer");
            self.stable.push_str(&renderer.push(delta));
            self.preview = renderer.snapshot_open_structures();
            *layouts = renderer.take_table_layouts();
            renderer.set_table_layouts(layouts.clone(), Some(budget));
            #[cfg(test)]
            {
                self.parsed_bytes += delta.len();
            }
            self.source.push_str(delta);
        }
        let mut output = String::with_capacity(self.stable.len() + self.preview.len());
        output.push_str(&self.stable);
        output.push_str(&self.preview);
        output.truncate(output.trim_end_matches('\n').len());
        output
    }
}
