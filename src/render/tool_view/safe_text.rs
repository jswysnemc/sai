//! 外部文本进入工具卡片前的控制符边界，不影响渲染器生成的样式。

/// 【终端】【安全正文】移除 ANSI，转义回车及其他不可显示控制符，保留换行与制表符。
/// 参数: `text` 为外部正文；返回: 可供渲染器处理的纯文本。
pub(super) fn plain(text: &str) -> String {
    let mut output = String::new();
    for ch in crate::render::ansi_clip::strip_ansi(text).chars() {
        if ch.is_control() && !matches!(ch, '\n' | '\t') {
            output.extend(ch.escape_default());
        } else {
            output.push(ch);
        }
    }
    output
}
