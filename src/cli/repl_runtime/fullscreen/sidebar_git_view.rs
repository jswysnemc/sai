//! 紧凑文件清单与真实增删统计。

use super::sidebar::row;
use super::sidebar_action::{SidebarAction, SidebarSection};
use super::sidebar_git::GitSummary;
use super::sidebar_palette::{fg, SidebarPalette};
use super::sidebar_view::SidebarRow;
use super::sidebar_widgets::{changes, columns, heading, item, safe_text};

/// 【终端侧栏】【Changes 分组】标题显示累计增删，文件只占一行且统计右对齐。
/// 参数: git 为快照，collapsed 为折叠状态，width 为宽度；返回: 分组内容。
pub(super) fn rows(git: &GitSummary, collapsed: bool, width: usize) -> Vec<SidebarRow> {
    let count = format!("{} {}", git.files.len(), totals(git));
    let mut rows = vec![row(
        heading("Changes", &count, collapsed, width),
        Some(SidebarAction::Toggle(SidebarSection::Git)),
    )];
    if collapsed {
        return rows;
    }
    match git.state.as_str() {
        "ready" if git.files.is_empty() => {
            rows.push(row(item("Working tree clean", width, true), None))
        }
        "ready" => {
            for (index, (status, path)) in git.files.iter().enumerate() {
                rows.push(row(
                    file(git, status, path, width),
                    Some(SidebarAction::GitFile(index)),
                ));
            }
        }
        "not_repo" => rows.push(row(item("Not a Git repository", width, true), None)),
        "error" => rows.push(row(item("Git status unavailable", width, true), None)),
        _ => rows.push(row(item("Checking working tree...", width, true), None)),
    }
    rows
}

/// 【终端审查】【文件行】文件名称与增删统计分离排版，不显示多行目录详情。
/// 参数: git 为快照，status 为文件状态，path 为路径，width 为宽度；返回: 文件行。
pub(super) fn file(git: &GitSummary, status: &str, path: &str, width: usize) -> String {
    let palette = SidebarPalette::current();
    let name = path.rsplit('/').next().unwrap_or(path);
    let marker = if status == "UU" {
        "!"
    } else if status.contains('D') {
        "D"
    } else if status == "??" || status.contains('A') {
        "A"
    } else if status.contains('R') {
        "R"
    } else {
        "M"
    };
    let label = format!("{}{marker} {}", fg(palette.muted), safe_text(name));
    item(
        &columns(&label, &changes(git.changes.get(path)), width),
        width,
        false,
    )
}

/// 【终端审查】【累计统计】汇总当前快照中真实的文本增删数量。
/// 参数: git 为快照；返回: 着色的累计增删。
pub(super) fn totals(git: &GitSummary) -> String {
    let total = git
        .files
        .iter()
        .filter_map(|(_, path)| git.changes.get(path))
        .fold(crate::web::GitFileStats::default(), |mut total, file| {
            total.added = total.added.saturating_add(file.added);
            total.removed = total.removed.saturating_add(file.removed);
            total
        });
    changes((git.files.is_empty() || !git.changes.is_empty()).then_some(&total))
}
