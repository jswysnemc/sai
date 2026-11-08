use super::*;

/// 【终端侧栏】【细线回归】静止、悬停和拖动均不出现中间粗线，分段绘制保持一致。
/// 参数: 无；返回: 无。
#[test]
fn divider_stays_uniform_in_all_interaction_states() {
    for (hovered, dragging) in [(false, false), (true, false), (true, true)] {
        let control = SidebarControl {
            divider_hovered: hovered,
            dragging,
            ..Default::default()
        };
        let source = "abc│def".to_string();
        let mut rows = vec![source.clone(); 20];
        decorate_divider(&control, &mut rows, 3, 0, 20);
        assert!(rows.windows(2).all(|pair| pair[0] == pair[1]));
        assert!(rows.iter().all(|row| !row.contains('┃')));
        assert_eq!(crate::render::ansi_clip::strip_ansi(&rows[10]), source);
        let mut tail = vec![source; 5];
        decorate_divider(&control, &mut tail, 3, 15, 20);
        assert_eq!(tail, rows[15..]);
    }
}
