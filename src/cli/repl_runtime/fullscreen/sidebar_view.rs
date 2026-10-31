//! 面板组合与侧栏绘制，领域条目由各自组件生成。

use super::layout::FullscreenLayout;
use super::sidebar::Sidebar;
use super::sidebar_action::{SidebarAction, SidebarSections};
use super::sidebar_git::GitSummary;
use super::sidebar_palette::{fg, SidebarPalette};
use super::sidebar_regions::{regions, SidebarPanel};
use super::sidebar_style;
use super::sidebar_widgets;
use crate::render::transcript::{SubagentOverviewEntry, TodoSnapshotItem};

#[derive(Default)]
pub(super) struct SidebarDocument {
    pub(super) rows: Vec<SidebarRow>,
    pub(super) todo_compact: bool,
    pub(super) panels: Vec<SidebarPanel>,
}

pub(super) struct SidebarRow {
    pub(super) text: String,
    pub(super) action: Option<SidebarAction>,
    pub(super) selected: bool,
}

pub(super) struct SidebarView<'a> {
    pub(super) viewing: Option<&'a str>,
    pub(super) compact: bool,
    pub(super) sections: &'a SidebarSections,
    pub(super) width: usize,
    pub(super) frame: usize,
}

/// 【终端侧栏】【面板组合】组合独立领域面板，布局与动作使用同一份条目模型。
/// 参数: agents 为子任务，todos 为待办，git 为快照，view 为排版配置；返回: 侧栏文档。
pub(super) fn document(
    agents: &[SubagentOverviewEntry],
    todos: &[TodoSnapshotItem],
    git: &GitSummary,
    view: SidebarView<'_>,
) -> SidebarDocument {
    let mut rows = Vec::new();
    let mut panels = Vec::new();
    // 1. 【终端侧栏】【独立区域】记录各分组来源，不把整块侧栏作为一个滚动文档
    for (section, panel) in [
        (
            super::sidebar_action::SidebarSection::Agents,
            super::sidebar_agents_view::rows(
                agents,
                view.viewing,
                view.sections.agents_collapsed,
                view.width,
            ),
        ),
        (
            super::sidebar_action::SidebarSection::Git,
            super::sidebar_git_view::rows(git, view.sections.git_collapsed, view.width),
        ),
        (
            super::sidebar_action::SidebarSection::Todo,
            super::sidebar_todo_view::rows(todos, view.compact, view.width, view.frame),
        ),
    ] {
        let header = rows.len();
        rows.extend(panel);
        panels.push(SidebarPanel {
            section,
            header,
            content: header + 1..rows.len(),
        });
    }
    SidebarDocument {
        rows,
        panels,
        todo_compact: view.compact,
    }
}

/// 【终端侧栏】【组件绘制】显示独立面板、鼠标状态与键盘焦点，不覆盖正文和输入框。
/// 参数: sidebar 为状态，layout 为分区，rows 为当前绘制行；返回: 无。
pub(super) fn overlay(sidebar: &Sidebar, layout: &FullscreenLayout, rows: &mut [String]) {
    let Some(geometry) = layout.sidebar else {
        return;
    };
    if layout.review {
        return;
    }
    let width = usize::from(geometry.width.saturating_sub(3));
    let height = usize::from(layout.composer_top.saturating_sub(1));
    let palette = SidebarPalette::current();
    let panels = regions(&sidebar.document, usize::from(layout.composer_top));
    let focused_action = sidebar
        .focused_row
        .and_then(|index| sidebar.document.rows.get(index))
        .and_then(|row| row.action);
    for (screen_row, output) in rows.iter_mut().enumerate() {
        let index = panels
            .iter()
            .find_map(|panel| panel.index_at(screen_row, &sidebar.scrolls));
        let entry = index.and_then(|index| sidebar.document.rows.get(index));
        let text = if screen_row == height {
            sidebar_style::footer(sidebar, height)
        } else if let Some(entry) = entry {
            let previewed = match entry.action {
                Some(SidebarAction::GitFile(file)) => sidebar
                    .git
                    .summary
                    .files
                    .get(file)
                    .is_some_and(|(_, path)| sidebar.preview.path.as_deref() == Some(path)),
                _ => false,
            };
            if entry.selected
                || previewed
                || (sidebar.focused && entry.action.is_some() && entry.action == focused_action)
            {
                sidebar_widgets::highlight(&entry.text, true)
            } else if sidebar.hovered == index {
                sidebar_widgets::highlight(&entry.text, false)
            } else {
                entry.text.clone()
            }
        } else {
            String::new()
        };
        let text = sidebar_style::clip(&text, width);
        let padding = width.saturating_sub(crate::cli::repl_text::visible_width(&text));
        output.push_str(&format!(
            "\x1b[0m{}│\x1b[0m {text}{} ",
            fg(palette.border),
            " ".repeat(padding)
        ));
    }
}
