//! 侧栏分组折叠与会话导航的独立动作定义。

use crate::cli::repl_runtime::panel_mouse::PanelAction;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SidebarSection {
    Agents,
    Git,
    Todo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SidebarAction {
    Toggle(SidebarSection),
    Panel(PanelAction),
    GitFile(usize),
}

#[derive(Default)]
pub(super) struct SidebarSections {
    pub(super) agents_collapsed: bool,
    pub(super) git_collapsed: bool,
}

impl SidebarSections {
    /// 【终端侧栏】【分组切换】切换目标分组，其他分组保持原状。
    /// 参数: section 为目标分组；返回: 无。
    pub(super) fn toggle(&mut self, section: SidebarSection) {
        let collapsed = match section {
            SidebarSection::Agents => &mut self.agents_collapsed,
            SidebarSection::Git => &mut self.git_collapsed,
            SidebarSection::Todo => return,
        };
        *collapsed = !*collapsed;
    }
}
