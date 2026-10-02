use html2md::{Handle, StructuredPrinter, TagHandler, TagHandlerFactory};
use std::collections::HashMap;

/// 【网页读取】【正文过滤】在 HTML 解析树上跳过非正文节点，避免把脚本和 CSS 当作 Markdown 返回。
struct HiddenContent;

impl TagHandlerFactory for HiddenContent {
    /// 【网页读取】【处理器构造】无参数，返回用于单个不可见节点的跳过处理器。
    fn instantiate(&self) -> Box<dyn TagHandler> {
        Box::new(Self)
    }
}

impl TagHandler for HiddenContent {
    /// 【网页读取】【隐藏节点】参数为 HTML 节点与输出器；不生成任何文本，无返回值。
    fn handle(&mut self, _tag: &Handle, _printer: &mut StructuredPrinter) {}

    /// 【网页读取】【节点收尾】参数为输出器；隐藏节点没有追加文本，无返回值。
    fn after_handle(&mut self, _printer: &mut StructuredPrinter) {}

    /// 【网页读取】【子树跳过】无参数，返回 true 以排除整个不可见子树。
    fn skip_descendants(&self) -> bool {
        true
    }
}

/// 【网页读取】【Markdown 正文】参数为 HTML，返回不含页面脚本、样式及元数据的 Markdown。
pub(super) fn markdown(html: &str) -> String {
    let handlers: HashMap<String, Box<dyn TagHandlerFactory>> =
        ["head", "script", "style", "template"]
            .into_iter()
            .map(|name| {
                (
                    name.to_string(),
                    Box::new(HiddenContent) as Box<dyn TagHandlerFactory>,
                )
            })
            .collect();
    html2md::parse_html_custom(html, &handlers)
}
