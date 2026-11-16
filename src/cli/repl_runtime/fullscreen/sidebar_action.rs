//! 侧栏分组折叠、手风琴聚焦与会话导航的独立动作定义。

use crate::cli::repl_runtime::panel_mouse::PanelAction;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SidebarSection {
    Sessions,
    Agents,
    Git,
    Todo,
    Background,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SidebarAction {
    Session(u64),
    Toggle(SidebarSection),
    Panel(PanelAction),
    GitFile(usize),
    Background(Option<usize>),
}

#[derive(Default)]
pub(super) struct SidebarSections {
    pub(super) sessions_collapsed: bool,
    pub(super) agents_collapsed: bool,
    pub(super) git_collapsed: bool,
    pub(super) todo_collapsed: bool,
    pub(super) background_collapsed: bool,
    /// 手风琴聚焦分组；有值时只展开目标分组，其余分组折叠为单行摘要
    pub(super) focused_section: Option<SidebarSection>,
}

impl SidebarSections {
    /// 【终端侧栏】【分组切换】切换目标分组，若处于手风琴模式且点击折叠分组则切换聚焦。
    /// 参数: section 为目标分组；返回: 无。
    pub(super) fn toggle(&mut self, section: SidebarSection) {
        if let Some(focused) = self.focused_section {
            if focused != section {
                self.focused_section = Some(section);
                self.set_collapsed(section, false);
                return;
            }
        }
        let collapsed = match section {
            SidebarSection::Sessions => &mut self.sessions_collapsed,
            SidebarSection::Agents => &mut self.agents_collapsed,
            SidebarSection::Git => &mut self.git_collapsed,
            SidebarSection::Todo => &mut self.todo_collapsed,
            SidebarSection::Background => &mut self.background_collapsed,
        };
        *collapsed = !*collapsed;
    }

    /// 【终端侧栏】【手风琴聚焦】在手风琴聚焦与自适应均衡模式之间切换目标分组。
    /// 参数: section 为目标分组；返回: 无。
    pub(super) fn toggle_focus(&mut self, section: SidebarSection) {
        if self.focused_section == Some(section) {
            self.focused_section = None;
        } else {
            self.focused_section = Some(section);
            self.set_collapsed(section, false);
        }
    }

    /// 【终端侧栏】【折叠判断】结合手风琴聚焦状态判定分组是否需要折叠展示。
    /// 参数: section 为分组标识；返回: true 表示折叠。
    pub(super) fn is_collapsed(&self, section: SidebarSection) -> bool {
        if let Some(focused) = self.focused_section {
            return focused != section;
        }
        match section {
            SidebarSection::Sessions => self.sessions_collapsed,
            SidebarSection::Agents => self.agents_collapsed,
            SidebarSection::Git => self.git_collapsed,
            SidebarSection::Todo => self.todo_collapsed,
            SidebarSection::Background => self.background_collapsed,
        }
    }

    /// 【终端侧栏】【显式设值】直接设置某分组的折叠开关。
    /// 参数: section 为分组标识，value 为折叠状态；返回: 无。
    fn set_collapsed(&mut self, section: SidebarSection, value: bool) {
        match section {
            SidebarSection::Sessions => self.sessions_collapsed = value,
            SidebarSection::Agents => self.agents_collapsed = value,
            SidebarSection::Git => self.git_collapsed = value,
            SidebarSection::Todo => self.todo_collapsed = value,
            SidebarSection::Background => self.background_collapsed = value,
        }
    }
}
