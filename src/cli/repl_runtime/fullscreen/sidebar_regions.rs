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
    pub(super) agents: usize,
    pub(super) git: usize,
    pub(super) todo: usize,
}

impl SidebarScrolls {
    /// 【终端侧栏】【独立偏移】读取指定分组的滚动位置。
    /// 参数: section 为分组；返回: 该分组偏移。
    pub(super) fn get(&self, section: SidebarSection) -> usize {
        match section {
            SidebarSection::Agents => self.agents,
            SidebarSection::Git => self.git,
            SidebarSection::Todo => self.todo,
        }
    }
    /// 【终端侧栏】【局部滚动】仅修改目标区域的滚动位置，不移动标题或其他分组。
    /// 参数: region 为目标区域，delta 为滚动量；返回: 是否变化。
    pub(super) fn scroll(&mut self, region: &SidebarRegion, delta: isize) -> bool {
        let offset = match region.section {
            SidebarSection::Agents => &mut self.agents,
            SidebarSection::Git => &mut self.git,
            SidebarSection::Todo => &mut self.todo,
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
            SidebarSection::Agents => &mut self.agents,
            SidebarSection::Git => &mut self.git,
            SidebarSection::Todo => &mut self.todo,
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

/// 【终端侧栏】【区域分配】三个标题固定，子任务和待办限制高度，Changes 使用剩余内容区。
/// 参数: document 为分组文档，height 为侧栏高度；返回: 分组布局。
pub(super) fn regions(document: &SidebarDocument, height: usize) -> Vec<SidebarRegion> {
    let available = height.saturating_sub(1 + document.panels.len());
    let len = |section| {
        document
            .panels
            .iter()
            .find(|panel| panel.section == section)
            .map_or(0, |panel| panel.content.len())
    };
    let agents = len(SidebarSection::Agents)
        .min(5)
        .min(available / 3 + usize::from(available > 0));
    let todo = len(SidebarSection::Todo)
        .min(4)
        .min(available.saturating_sub(agents) / 2);
    let git = len(SidebarSection::Git).min(available.saturating_sub(agents + todo));
    let mut top = 0;
    document
        .panels
        .iter()
        .map(|panel| {
            let visible = match panel.section {
                SidebarSection::Agents => agents,
                SidebarSection::Git => git,
                SidebarSection::Todo => todo,
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
