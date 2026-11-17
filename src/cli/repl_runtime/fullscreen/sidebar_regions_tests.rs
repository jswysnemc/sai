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

/// 【侧栏测试】【后台滚动】后台命令使用有界区域，滚动时不改变其他分组偏移。
/// 参数: 无；返回: 无。
#[test]
fn background_commands_scroll_independently_with_a_bounded_preview() {
    let mut document = document([20, 50, 30]);
    let header = document.panels.last().unwrap().content.end;
    document.panels.push(SidebarPanel {
        section: SidebarSection::Background,
        header,
        content: header + 1..header + 31,
    });
    for height in [8, 12, 16, 30, 60] {
        let regions = regions(&document, height);
        let background = regions.last().unwrap();
        assert!((1..=4).contains(&background.visible));
        assert!(background.top + background.visible < height - 1);
        let mut scrolls = SidebarScrolls::default();
        assert!(scrolls.scroll(background, 3));
        assert_eq!((scrolls.agents, scrolls.git, scrolls.todo), (0, 0, 0));
        assert_eq!(background.index_at(background.top, &scrolls), Some(header));
        assert_eq!(
            background.index_at(background.top + 1, &scrolls),
            Some(header + 4)
        );
    }
}

/// 【侧栏测试】【余量让渡】其余分组简短时，会话列表突破 6 行上限接管底部空白；手风琴聚焦可单区占满。
/// 参数: 无；返回: 无。
#[test]
fn surplus_height_expands_sessions_and_accordion_focus_isolates_section() {
    use super::sidebar_regions::regions_with_focus;
    let panels = [
        (SidebarSection::Agents, 0usize),
        (SidebarSection::Todo, 0usize),
        (SidebarSection::Git, 0usize),
        (SidebarSection::Sessions, 24usize),
        (SidebarSection::Background, 1usize),
    ];
    let mut offset = 0;
    let built = panels
        .into_iter()
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
    let doc = SidebarDocument {
        panels: built,
        ..Default::default()
    };

    let balanced = regions(&doc, 44);
    let sessions = balanced
        .iter()
        .find(|region| region.section == SidebarSection::Sessions)
        .unwrap();
    let background = balanced
        .iter()
        .find(|region| region.section == SidebarSection::Background)
        .unwrap();
    assert_eq!(sessions.visible, 24);
    assert_eq!(background.top, sessions.top + 1 + sessions.visible + 1);

    let tall_plan = super::sidebar_regions::plan_vertical(&doc, 62, None, None);
    let tall_sessions = tall_plan
        .regions
        .iter()
        .find(|region| region.section == SidebarSection::Sessions)
        .unwrap();
    let tall_bg = tall_plan
        .regions
        .iter()
        .find(|region| region.section == SidebarSection::Background)
        .unwrap();
    assert!(
        tall_plan.ideal_shift >= 5 && tall_plan.chrome.header_top >= 2,
        "高屏盈余高度应下移顶栏与分组使会话向侧栏纵向视觉中线收拢"
    );
    let sessions_mid = tall_sessions.top + (1 + tall_sessions.visible) / 2;
    assert!(
        (24..=38).contains(&sessions_mid),
        "62 行侧栏中会话区块中点应贴近侧栏纵向中线: mid={sessions_mid}"
    );
    assert_eq!(
        tall_bg.top,
        tall_sessions.top + 1 + tall_sessions.visible + 1,
        "后台任务应自然紧随会话列表而非孤立流放到底部"
    );

    let focused = regions_with_focus(&doc, 44, Some(SidebarSection::Sessions));
    assert!(focused
        .iter()
        .filter(|r| r.section != SidebarSection::Sessions)
        .all(|r| r.visible == 0));
    let focused_sessions = focused
        .iter()
        .find(|r| r.section == SidebarSection::Sessions)
        .unwrap();
    assert_eq!(focused_sessions.visible, 24);
}
