//! 各分组的固定标题、可见范围、纵向视觉中线排布与独立滚动状态。

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

/// 侧栏纵向基准行号：统一支配顶栏、上下文指标与可滚动分组的屏幕坐标。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SidebarChromeRows {
    pub(super) header_top: usize,
    pub(super) metrics_top: usize,
    pub(super) sections_top: usize,
}

/// 侧栏单帧完整纵向布局结果。
pub(super) struct SidebarVerticalPlan {
    pub(super) chrome: SidebarChromeRows,
    pub(super) regions: Vec<SidebarRegion>,
    pub(super) surplus: usize,
    pub(super) ideal_shift: usize,
}

#[derive(Default)]
pub(super) struct SidebarScrolls {
    pub(super) sessions: usize,
    pub(super) agents: usize,
    pub(super) groups: usize,
    pub(super) git: usize,
    pub(super) todo: usize,
    pub(super) background: usize,
}

impl SidebarScrolls {
    /// 【终端侧栏】【偏移更新】设置指定分组偏移，由调用方按区域可见范围夹紧。
    /// 参数: section 为分组，offset 为有效偏移；返回: 无。
    pub(super) fn set(&mut self, section: SidebarSection, offset: usize) {
        match section {
            SidebarSection::Sessions => self.sessions = offset,
            SidebarSection::Agents => self.agents = offset,
            SidebarSection::Groups => self.groups = offset,
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
            SidebarSection::Groups => self.groups,
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
            SidebarSection::Groups => &mut self.groups,
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
            SidebarSection::Groups => &mut self.groups,
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

/// 【终端侧栏】【呼吸留白判定】高屏在指标区下方与底栏上方各预留一行透明呼吸空白，矮屏让渡给列表正文。
/// 参数: height 为侧栏高度；返回: 是否预留顶底呼吸空行。
pub(super) fn has_breathing_gaps(height: usize) -> bool {
    height >= 22
}

/// 【终端侧栏】【区域分配】默认按黄金比例将盈余高度收拢至侧栏纵向视觉中线；供测试调用。
/// 参数: document 为分组文档，height 为侧栏高度；返回: 分组布局。
#[cfg(test)]
pub(super) fn regions(document: &SidebarDocument, height: usize) -> Vec<SidebarRegion> {
    plan_vertical(document, height, None, None).regions
}

/// 【终端侧栏】【聚焦分配】计算带手风琴焦点的默认居中分组布局。
/// 参数: document 为分组文档，height 为侧栏高度，focused_section 为可选手风琴焦点；返回: 分组布局。
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn regions_with_focus(
    document: &SidebarDocument,
    height: usize,
    focused_section: Option<SidebarSection>,
) -> Vec<SidebarRegion> {
    plan_vertical(document, height, focused_section, None).regions
}

/// 【终端侧栏】【完整纵向规划】计算顶栏、指标与分组位置，将盈余留白按约 40%/60% 垫入上缘使内容向侧栏纵向视觉中线靠拢。
/// 参数: document 为分组文档，height 为总高度，focused_section 为手风琴焦点，locked_shift 为指针驻留期间锁定的偏移；返回: 纵向规划。
pub(super) fn plan_vertical(
    document: &SidebarDocument,
    height: usize,
    focused_section: Option<SidebarSection>,
    locked_shift: Option<usize>,
) -> SidebarVerticalPlan {
    let header_len = super::sidebar_header_path::content_height(height);
    let top_sink = usize::from(header_len > 0);
    let metrics_len = super::sidebar_metrics::height(height);
    let gaps = usize::from(has_breathing_gaps(height));
    let base_header = top_sink + header_len + metrics_len + gaps;
    let footer = 1 + gaps;
    let body_end = height.saturating_sub(footer);
    let panel_count = document.panels.len();
    let raw_available = body_end.saturating_sub(base_header + panel_count);

    // 1. 【终端侧栏】【紧凑分组】中等高度归还分组空行给清单，高屏保留一行分组留白
    let separators = if focused_section.is_none() && height >= 34 && panel_count > 1 {
        (panel_count - 1).min(raw_available / 2)
    } else {
        0
    };
    let available = raw_available.saturating_sub(separators);
    let len = |section| {
        document
            .panels
            .iter()
            .find(|panel| panel.section == section)
            .map_or(0, |panel| panel.content.len())
    };

    // 2. 【终端侧栏】【手风琴模式】单区聚焦时将其余非焦点区域收敛至 0 行，释放全屏高度
    let (mut agents, mut groups, mut todo, mut git, mut sessions, background) =
        if let Some(target) = focused_section {
            (
                usize::from(target == SidebarSection::Agents)
                    * len(SidebarSection::Agents).min(available),
                usize::from(target == SidebarSection::Groups)
                    * len(SidebarSection::Groups).min(available),
                usize::from(target == SidebarSection::Todo)
                    * len(SidebarSection::Todo).min(available),
                usize::from(target == SidebarSection::Git)
                    * len(SidebarSection::Git).min(available),
                usize::from(target == SidebarSection::Sessions)
                    * len(SidebarSection::Sessions).min(available),
                usize::from(target == SidebarSection::Background)
                    * len(SidebarSection::Background).min(available),
            )
        } else {
            // 3. 【终端侧栏】【保底分配】后台入口与短分组优先保底，长文件与待办均衡共享
            let background = len(SidebarSection::Background)
                .min(4)
                .min((available / 3).max(1))
                .min(available);
            let rem = available.saturating_sub(background);
            let groups = len(SidebarSection::Groups)
                .min(4)
                .min((rem / 3).max(usize::from(len(SidebarSection::Groups) > 0)))
                .min(rem);
            let rem = rem.saturating_sub(groups);
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
            (agents, groups, todo, git, sessions, background)
        };

    // 4. 【终端侧栏】【余量回收】剩余空间优先归还会话、变更、群组与待办列表
    if focused_section.is_none() {
        let used = agents + groups + todo + git + sessions + background;
        let mut leftover = available.saturating_sub(used);
        if leftover > 0 {
            for (slot, max_len) in [
                (&mut sessions, len(SidebarSection::Sessions)),
                (&mut git, len(SidebarSection::Git)),
                (&mut agents, len(SidebarSection::Agents)),
                (&mut groups, len(SidebarSection::Groups)),
                (&mut todo, len(SidebarSection::Todo)),
            ] {
                let extra = max_len.saturating_sub(*slot).min(leftover);
                *slot += extra;
                leftover -= extra;
            }
        }
    }

    // 5. 【终端侧栏】【纵向视觉中线】将纵向盈余按约 2:3 光学比例分配到顶栏与分组上缘，使会话与列表围绕侧栏纵向中线排布
    let visible_for = |section| match section {
        SidebarSection::Sessions => sessions,
        SidebarSection::Agents => agents,
        SidebarSection::Groups => groups,
        SidebarSection::Git => git,
        SidebarSection::Todo => todo,
        SidebarSection::Background => background,
    };
    let total_visible: usize = document
        .panels
        .iter()
        .map(|panel| visible_for(panel.section))
        .sum();
    let occupied = panel_count + separators + total_visible;
    let surplus = body_end
        .saturating_sub(base_header)
        .saturating_sub(occupied);
    let ideal_shift = if focused_section.is_none() && height >= 26 {
        ((surplus * 2) / 5).min(14)
    } else {
        0
    };
    let applied_shift = if focused_section.is_some() {
        0
    } else {
        locked_shift.unwrap_or(ideal_shift).min(surplus)
    };
    let extra_top_sink = if header_len > 0 {
        (applied_shift / 3).min(3)
    } else {
        0
    };
    let extra_section_sink = applied_shift - extra_top_sink;
    let header_top = top_sink + extra_top_sink;
    let metrics_top = header_top + header_len;
    let sections_top = metrics_top + metrics_len + gaps + extra_section_sink;

    let mut remaining_seps = separators;
    let mut top = sections_top;
    let regions = document
        .panels
        .iter()
        .enumerate()
        .map(|(idx, panel)| {
            let visible = visible_for(panel.section);
            if idx > 0 && remaining_seps > 0 {
                top += 1;
                remaining_seps -= 1;
            }
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
        .collect();

    SidebarVerticalPlan {
        chrome: SidebarChromeRows {
            header_top,
            metrics_top,
            sections_top,
        },
        regions,
        surplus,
        ideal_shift,
    }
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
