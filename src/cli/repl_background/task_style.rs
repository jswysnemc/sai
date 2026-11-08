//! 后台任务状态的显示文字与配色，侧栏概览和管理面板共用同一套映射。

use crate::render::panel_palette::{PanelPalette, Rgb};

/// 【后台任务】【状态样式】把服务端状态映射为短词与主题色，运行中为强调色，失败与超时为警示色。
/// 参数: status 为任务状态字段，palette 为当前主题；返回: 状态短词与前景色。
pub(in crate::cli) fn status_style(status: &str, palette: &PanelPalette) -> (&'static str, Rgb) {
    match status {
        "running" => ("running", palette.accent),
        "failed" => ("failed", palette.removed),
        "timed_out" => ("timeout", palette.removed),
        "stopped" => ("stopped", palette.muted),
        _ => ("exited", palette.muted),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【后台任务测试】【状态样式】未知状态按已退出处理，警示色只用于失败和超时。
    /// 参数: 无；返回: 无。
    #[test]
    fn unknown_status_is_muted_and_failures_use_removed_color() {
        let palette = PanelPalette::for_background((20, 22, 24));
        assert_eq!(
            status_style("running", &palette),
            ("running", palette.accent)
        );
        assert_eq!(status_style("timed_out", &palette).1, palette.removed);
        assert_eq!(status_style("queued", &palette), ("exited", palette.muted));
    }
}
