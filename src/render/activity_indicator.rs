//! 运行状态共用的单列圆环指示器，不改变相邻文字对齐。

/// 【终端状态】【运行指示】参数 frame 为时间帧号；返回主题强调色单列圆环，每两帧旋转一次
pub(crate) fn render_activity_indicator(frame: usize) -> String {
    let glyph = ["◜", "◝", "◞", "◟"][(frame / 2) % 4];
    format!(
        "{}{glyph}\x1b[0m",
        crate::render::panel_palette::fg(
            crate::render::panel_palette::PanelPalette::current().accent
        )
    )
}

#[cfg(test)]
mod tests {
    /// 【终端状态】【圆环回归】无参数；返回无，确保所有帧等宽且不再使用散点字形
    #[test]
    fn activity_indicator_is_a_single_column_ring() {
        for frame in 0..16 {
            let rendered = super::render_activity_indicator(frame);
            let text = crate::render::ansi_clip::strip_ansi(&rendered);
            assert_eq!(crate::render::ansi_clip::ansi_visible_width(&rendered), 1);
            assert!(["◜", "◝", "◞", "◟"].contains(&text.as_str()));
        }
    }
}
