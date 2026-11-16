//! 各分组的固定标题、可见范围和独立滚动状态。

use super::sidebar_action::SidebarSection;
use super::sidebar_view::SidebarDocument;
use std::ops::Range;

pub(super) struct SidebarPanel {
    pub(super) section: SidebarSection,
    pub(super) header: usize,
    pub(super) content: Range<usize>,
}

pub(super) struct SidebarRegion {
    pub(super) section: SidebarSection,
    pub(super) top: usize,
    pub(super) header: usize,
    pub(super) content: Range<usize>,
    pub(super) visible: usize,
}

#[derive(Default)]
pub(super) struct SidebarScrolls {
    pub(super) sessions: usize,
    pub(super) agents: usize,
    pub(super) git: usize,
    pub(super) todo: usize,
    pub(super) background: usize,
}

impl SidebarScrolls {
    /// 【终端侧栏】【偏移更新】设置指定分组偏移，由调用方按区域可见范围夹紧
    /// 参数：section 为分组，offset 为有效偏移；返回：无
    pub(super) fn set(&mut self, section: SidebarSection, offset: usize) {
        match section {
            SidebarSection::Sessions => self.sessions = offset,
            SidebarSection::Agents => self.agents = offset,
            SidebarSection::Git => self.git = offset,
            SidebarSection::Todo => self.todo = offset,
            SidebarSection::Background => self.background = offset,
        }
    }

    /// 【终端侧栏】【独立偏移】读取指定分组的滚动位置。
    /// 参数: section 为分组；返回: 该分组偏移。
    pub(super) fn get(&self, section: SidebarSection) -> usize {
        match section {
            SidebarSection::Sessions => self.sessions,
            SidebarSection::Agents => self.agents,
            SidebarSection::Git => self.git,
            SidebarSection::Todo => self.todo,
            SidebarSection::Background => self.background,
        }
    }

    /// 【终端侧栏】【局部滚动】仅修改目标区域的滚动位置，不移动标题或其他分组。
    /// 参数: region 为目标区域，delta 为滚动量；返回: 是否变化。
    pub(super) fn scroll(&mut self, region: &SidebarRegion, delta: isize) -> bool {
        let offset = match region.section {
            SidebarSection::Sessions => &mut self.sessions,
            SidebarSection::Agents => &mut self.agents,
            SidebarSection::Git => &mut self.git,
            SidebarSection::Todo => &mut self.todo,
            SidebarSection::Background => &mut self.background,
        };
        let before = *offset;
        *offset = offset
            .saturating_add_signed(delta)
            .min(region.content.len().saturating_sub(region.visible));
        before != *offset
    }

    /// 【终端侧栏】【焦点可见】只调整目标条目所属分组，保持其他区域偏移。
    /// 参数: region 为所属区域，index 为文档行；返回: 无。
    pub(super) fn reveal(&mut self, region: &SidebarRegion, index: usize) {
        if !region.content.contains(&index) {
            return;
        }
        let offset = match region.section {
            SidebarSection::Sessions => &mut self.sessions,
            SidebarSection::Agents => &mut self.agents,
            SidebarSection::Git => &mut self.git,
            SidebarSection::Todo => &mut self.todo,
            SidebarSection::Background => &mut self.background,
        };
        let target = index - region.content.start;
        if target < *offset {
            *offset = target;
        }
        if target >= *offset + region.visible {
            *offset = target + 1 - region.visible.max(1);
        }
        *offset = (*offset).min(region.content.len().saturating_sub(region.visible));
    }
}

/// 【终端侧栏】【极细线判定】高屏预留顶栏与底栏呼吸分割线，矮屏保留所有行给列表正文。
/// 参数: height 为侧栏高度；返回: 是否绘制分割线。
pub(super) fn has_hairlines(height: usize) -> bool {
    height >= 22
}

/// 【终端侧栏】【区域分配】默认走双阶段弹性分配消除底部留白；供测试调用。
/// 参数: document 为分组文档，height 为侧栏高度；返回: 分组布局。
#[cfg(test)]
pub(super) fn regions(document: &SidebarDocument, height: usize) -> Vec<SidebarRegion> {
    regions_with_focus(document, height, None)
}

/// 【终端侧栏】【聚焦分配】支持手风琴单区聚焦（独占可用行）或均衡模式余量重分配。
/// 参数: document 为分组文档，height 为侧栏高度，focused_section 为可选手风琴焦点；返回: 分组布局。
pub(super) fn regions_with_focus(
    document: &SidebarDocument,
    height: usize,
    focused_section: Option<SidebarSection>,
) -> Vec<SidebarRegion> {
    let hairlines = usize::from(has_hairlines(height));
    let header =
        super::pane_header::height(height) + super::sidebar_metrics::height(height) + hairlines;
    let footer = 1 + hairlines;
    let available = height.saturating_sub(header + footer + document.panels.len());
    let len = |section| {
        document
            .panels
            .iter()
            .find(|panel| panel.section == section)
            .map_or(0, |panel| panel.content.len())
    };

    // 1. 【终端侧栏】【手风琴模式】单区聚焦时，将其余非焦点区域收敛至 0 行，释放全屏高度
    let (mut agents, mut todo, mut git, mut sessions, background) = if let Some(target) =
        focused_section
    {
        (
            usize::from(target == SidebarSection::Agents)
                * len(SidebarSection::Agents).min(available),
            usize::from(target == SidebarSection::Todo) * len(SidebarSection::Todo).min(available),
            usize::from(target == SidebarSection::Git) * len(SidebarSection::Git).min(available),
            usize::from(target == SidebarSection::Sessions)
                * len(SidebarSection::Sessions).min(available),
            usize::from(target == SidebarSection::Background)
                * len(SidebarSection::Background).min(available),
        )
    } else {
        // 2. 【终端侧栏】【阶段一：保底分配】后台入口与短分组优先保底，长文件与待办均衡共享
        let background = len(SidebarSection::Background)
            .min(4)
            .min((available / 3).max(1))
            .min(available);
        let rem = available.saturating_sub(background);
        let sessions = len(SidebarSection::Sessions)
            .min(6)
            .min(rem / 3 + usize::from(rem > 0));
        let rem = rem.saturating_sub(sessions);
        let agents = len(SidebarSection::Agents)
            .min(5)
            .min(rem / 3 + usize::from(rem > 0));
        let rem_todo_git = rem.saturating_sub(agents);
        let git_reserve = len(SidebarSection::Git).min(6).min(rem_todo_git / 2);
        let todo = len(SidebarSection::Todo).min(rem_todo_git.saturating_sub(git_reserve));
        let git = len(SidebarSection::Git).min(rem_todo_git.saturating_sub(todo));
        (agents, todo, git, sessions, background)
    };

    // 3. 【终端侧栏】【阶段二：余量回收】将空分组留出的多余行依次让渡给未展示完的活跃分组，避免底部留白
    if focused_section.is_none() {
        let used = agents + todo + git + sessions + background;
        let mut leftover = available.saturating_sub(used);
        if leftover > 0 {
            for (slot, max_len) in [
                (&mut sessions, len(SidebarSection::Sessions)),
                (&mut git, len(SidebarSection::Git)),
                (&mut agents, len(SidebarSection::Agents)),
                (&mut todo, len(SidebarSection::Todo)),
            ] {
                let extra = max_len.saturating_sub(*slot).min(leftover);
                *slot += extra;
                leftover -= extra;
            }
        }
    }

    let mut top = header;
    document
        .panels
        .iter()
        .map(|panel| {
            let visible = match panel.section {
                SidebarSection::Sessions => sessions,
                SidebarSection::Agents => agents,
                SidebarSection::Git => git,
                SidebarSection::Todo => todo,
                SidebarSection::Background => background,
            };
            let region = SidebarRegion {
                section: panel.section,
                top,
                header: panel.header,
                content: panel.content.clone(),
                visible,
            };
            top += 1 + visible;
            region
        })
        .collect()
}

impl SidebarRegion {
    /// 【终端侧栏】【行号映射】从固定屏幕区域映射到该分组文档行。
    /// 参数: row 为屏幕行，scrolls 为独立偏移；返回: 可见文档行。
    pub(super) fn index_at(&self, row: usize, scrolls: &SidebarScrolls) -> Option<usize> {
        if row == self.top {
            return Some(self.header);
        }
        if row <= self.top || row > self.top + self.visible {
            return None;
        }
        let index = self.content.start + scrolls.get(self.section) + row - self.top - 1;
        self.content.contains(&index).then_some(index)
    }

    /// 【终端侧栏】【屏幕定位】查找当前可见文档行的屏幕位置。
    /// 参数: index 为文档行，scrolls 为独立偏移；返回: 屏幕行。
    #[cfg(test)]
    pub(super) fn row_for(&self, index: usize, scrolls: &SidebarScrolls) -> Option<usize> {
        if index == self.header {
            return Some(self.top);
        }
        let start = self.content.start + scrolls.get(self.section);
        (index >= start && index < start + self.visible && self.content.contains(&index))
            .then(|| self.top + 1 + index - start)
    }
}
