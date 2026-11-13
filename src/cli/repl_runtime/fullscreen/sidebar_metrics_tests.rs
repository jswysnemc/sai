use super::sidebar_metrics::*;
/// 【终端】【指标降级】未知数据不伪造零值，短屏不占用操作入口
#[test]
fn unknown_metrics_and_short_screens_are_supported() {
    let metrics = SidebarMetrics::default();
    assert!(metrics.rows(30, 15).is_empty());
    let rows = metrics.rows(30, 30);
    assert_eq!(rows.len(), 3);
    assert!(crate::render::ansi_clip::strip_ansi(&rows[0]).contains("cache --"));
    for width in [1, 10, 25] {
        for line in metrics.rows(width, 30) {
            assert!(crate::render::ansi_clip::ansi_visible_width(&line) <= width);
        }
    }
}
