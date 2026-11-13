//! 直接写终端的设置与任务面板也使用统一对比度校正。

/// 【终端】【直接输出校正】参数为带 ANSI 的文本，返回符合实际终端背景的文字样式
pub(crate) fn readable_terminal_text(text: &str) -> String {
    super::terminal_text_style::correct_ansi_text(
        text,
        super::terminal_palette::terminal_default_colors(),
        super::terminal_palette::terminal_palette().true_color,
    )
}
