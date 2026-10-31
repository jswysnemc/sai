//! 待办列表可用高度与独立滚动的回归测试。

use super::sidebar_action::SidebarSection;
use super::sidebar_regions::{regions, SidebarPanel, SidebarScrolls};
use super::sidebar_view::SidebarDocument;

/// 【侧栏测试】【高度夹具】生成只包含区域范围的布局文档。
/// 参数: counts 依次为子任务、文件和待办行数；返回: 分组文档。
fn document(counts: [usize; 3]) -> SidebarDocument {
    let mut offset = 0;
    let panels = [
        SidebarSection::Agents,
        SidebarSection::Git,
        SidebarSection::Todo,
    ]
    .into_iter()
    .zip(counts)
    .map(|(section, count)| {
        let panel = SidebarPanel {
            section,
            header: offset,
            content: offset + 1..offset + 1 + count,
        };
        offset += count + 1;
        panel
    })
    .collect();
    SidebarDocument {
        panels,
        ..Default::default()
    }
}

/// 【侧栏测试】【完整清单】空间充足时不限制四行，其他分组短时归还空间。
/// 参数: 无；返回: 无，列表截断时断言失败。
#[test]
fn sufficient_height_shows_every_todo() {
    let document = document([1, 2, 12]);
    let regions = regions(&document, 30);
    assert_eq!(regions[2].visible, 12);
    assert_eq!(regions[1].visible, 2);
}

/// 【侧栏测试】【空间优先】文件很多时保留文件预览，待办仍优先完整展示。
/// 参数: 无；返回: 无，待办受固定高度限制时断言失败。
#[test]
fn todo_uses_height_without_starving_files() {
    let regions = regions(&document([1, 80, 10]), 30);
    assert_eq!(regions[2].visible, 10);
    assert!(regions[1].visible >= 6);
}

/// 【侧栏测试】【滚动隔离】空间不足只滚动待办，增高后恢复完整范围。
/// 参数: 无；返回: 无，区域越界或偏移串扰时断言失败。
#[test]
fn overflow_scrolls_locally_and_growth_reveals_all() {
    let document = document([3, 8, 20]);
    let small = regions(&document, 18);
    assert!(small.iter().all(|region| region.top + region.visible < 18));
    let mut scrolls = SidebarScrolls::default();
    assert!(scrolls.scroll(&small[2], 3));
    assert_eq!((scrolls.agents, scrolls.git), (0, 0));
    let tall = regions(&document, 45);
    assert_eq!(tall[2].visible, 20);
    scrolls.scroll(&tall[2], 0);
    assert_eq!(scrolls.todo, 0);
}
