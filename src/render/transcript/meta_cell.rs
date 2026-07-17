/// REPL 系统提示、控制命令与错误的 source 数据。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MetaCell {
    pub(crate) text: String,
}

/// 渲染系统提示或控制命令消息。
///
/// 参数:
/// - `cell`: 元信息源数据
///
/// 返回:
/// - ANSI 文本块
pub(crate) fn render(cell: &MetaCell) -> String {
    format!("\x1b[2m{}\x1b[0m", cell.text)
}
