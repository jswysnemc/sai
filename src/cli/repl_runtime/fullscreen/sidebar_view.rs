//! 面板组合与侧栏绘制，领域条目由各自组件生成。

use super::layout::FullscreenLayout;
use super::sidebar::Sidebar;
use super::sidebar_action::{SidebarAction, SidebarSection, SidebarSections};
use super::sidebar_control::SidebarControl;
use super::sidebar_git::GitSummary;
use super::sidebar_palette::{fg, SidebarPalette};
use super::sidebar_regions::{has_hairlines, regions_with_focus, SidebarPanel};
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
    pub(super) sessions: Option<&'a super::sidebar_sessions::SidebarSessions>,
    pub(super) viewing: Option<&'a str>,
    pub(super) compact: bool,
    pub(super) sections: &'a SidebarSections,
    pub(super) width: usize,
    pub(super) frame: usize,
}

/// 【终端侧栏】【面板组合】组合独立领域面板，支持手风琴聚焦状态传递。
/// 参数: agents 为子任务，todos 为待办，git 为快照，background 为后台任务，view 为排版配置；返回: 侧栏文档。
pub(super) fn document(
    agents: &[SubagentOverviewEntry],
    todos: &[TodoSnapshotItem],
    git: &GitSummary,
    background: &crate::cli::repl_background::EmbeddedTaskPanel,
    view: SidebarView<'_>,
) -> SidebarDocument {
    let mut rows = Vec::new();
    let mut panels = Vec::new();
    let todo_compact = view
        .sections
        .focused_section
        .map_or(view.compact, |focused| focused != SidebarSection::Todo);
    // 1. 【终端侧栏】【独立区域】当前执行、待办与变更优先，会话导航与后台管理紧随其后
    for (section, panel) in [
        (
            SidebarSection::Agents,
            super::sidebar_agents_view::rows(
                agents,
                view.viewing,
                view.sections.is_collapsed(SidebarSection::Agents),
                view.width,
                view.frame,
            ),
        ),
        (
            SidebarSection::Todo,
            super::sidebar_todo_view::rows(todos, todo_compact, view.width, view.frame),
        ),
        (
            SidebarSection::Git,
            super::sidebar_git_view::rows(
                git,
                view.sections.is_collapsed(SidebarSection::Git),
                view.width,
            ),
        ),
        (
            SidebarSection::Sessions,
            view.sessions.map_or_else(Vec::new, |sessions| {
                super::sidebar_sessions_view::rows(
                    sessions,
                    view.sections.is_collapsed(SidebarSection::Sessions),
                    view.width,
                )
            }),
        ),
        (
            SidebarSection::Background,
            super::sidebar_background_view::rows(
                background,
                view.sections.is_collapsed(SidebarSection::Background),
                view.width,
            ),
        ),
    ] {
        if panel.is_empty() {
            continue;
        }
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
        todo_compact,
    }
}

/// 【终端侧栏】【组件绘制】显示独立面板、鼠标状态与键盘焦点，不覆盖正文和输入框。
/// 参数: sidebar 为状态，layout 为分区，control 为控件状态，rows 为绘制行；返回: 无。
pub(super) fn overlay(
    sidebar: &Sidebar,
    layout: &FullscreenLayout,
    control: &SidebarControl,
    rows: &mut [String],
) {
    let Some(geometry) = layout.sidebar else {
        return;
    };
    if layout.review {
        return;
    }
    let palette = SidebarPalette::current();
    let panel = panel_rows(sidebar, layout, control);
    let width = usize::from(geometry.width.saturating_sub(3));
    for (row, output) in rows.iter_mut().enumerate() {
        let text = panel.get(row).map(String::as_str).unwrap_or_default();
        let padding = width.saturating_sub(crate::cli::repl_text::visible_width(text));
        output.push_str(&format!(
            "\x1b[0m{}│\x1b[0m {text}{} ",
            fg(palette.border),
            " ".repeat(padding)
        ));
    }
}

/// 【终端侧栏】【完整面板】品牌区、极细分隔线、独立列表和页脚覆盖右栏全高度，供正文与输入右侧共用。
/// 参数: sidebar 为文档与焦点，layout 为布局，control 为控件状态；返回: 不含分界线的面板行。
pub(super) fn panel_rows(
    sidebar: &Sidebar,
    layout: &FullscreenLayout,
    _control: &SidebarControl,
) -> Vec<String> {
    let Some(geometry) = layout.sidebar else {
        return Vec::new();
    };
    let width = usize::from(geometry.width.saturating_sub(3));
    let height = usize::from(layout.rows);
    let palette = SidebarPalette::current();
    let header = sidebar.metrics.heading(width, height);
    let metrics = sidebar.metrics.rows(width, height);
    let hairlines = has_hairlines(height);
    let top_hairline_row = hairlines.then_some(header.len() + metrics.len());
    let bottom_hairline_row = hairlines.then(|| height.saturating_sub(2));
    let panels = regions_with_focus(&sidebar.document, height, sidebar.sections.focused_section);
    let focused_action = sidebar
        .focused_row
        .and_then(|index| sidebar.document.rows.get(index))
        .and_then(|row| row.action);
    (0..height)
        .map(|screen_row| {
            let index = panels
                .iter()
                .find_map(|panel| panel.index_at(screen_row, &sidebar.scrolls));
            let entry = index.and_then(|index| sidebar.document.rows.get(index));
            let text = if let Some(line) = header.get(screen_row) {
                line.clone()
            } else if let Some(line) = screen_row
                .checked_sub(header.len())
                .and_then(|row| metrics.get(row))
            {
                line.clone()
            } else if Some(screen_row) == top_hairline_row
                || Some(screen_row) == bottom_hairline_row
            {
                sidebar_widgets::hairline(width)
            } else if screen_row + 1 == height {
                sidebar_style::footer(sidebar, width)
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
                let focused =
                    sidebar.focused && entry.action.is_some() && entry.action == focused_action;
                // 1. 【终端侧栏】【状态叠加】当前项加粗，键盘焦点使用底色，鼠标悬停只改变前景
                let text = if entry.selected || previewed || focused {
                    sidebar_widgets::highlight(&entry.text, true)
                } else {
                    entry.text.clone()
                };
                if focused {
                    sidebar_widgets::tint(&text, palette.focus_bg)
                } else if sidebar.hovered == index {
                    sidebar_widgets::highlight(&text, false)
                } else {
                    text
                }
            } else {
                String::new()
            };
            sidebar_style::clip(&text, width)
        })
        .collect()
}
