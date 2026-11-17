//! 紧凑文件清单与真实增删统计。

use super::sidebar::row;
use super::sidebar_action::{SidebarAction, SidebarSection};
use super::sidebar_git::GitSummary;
use super::sidebar_palette::{fg, SidebarPalette};
use super::sidebar_view::SidebarRow;
use super::sidebar_widgets::{changes, columns, heading_with_tone, item, safe_text};

/// 【终端侧栏】【Changes 分组】非仓库与净空状态单行降噪，避免假性零值增删与冗余提示行。
/// 参数: git 为快照，collapsed 为折叠状态，width 为宽度；返回: 分组内容。
pub(super) fn rows(git: &GitSummary, collapsed: bool, width: usize) -> Vec<SidebarRow> {
    let palette = SidebarPalette::current();
    let quiet =
        matches!(git.state.as_str(), "not_repo") || (git.state == "ready" && git.files.is_empty());
    let count = match git.state.as_str() {
        "not_repo" => format!(
            "{}{}\x1b[39m",
            fg(palette.subtle),
            super::sidebar_i18n::git_no_repo()
        ),
        "ready" if git.files.is_empty() => format!(
            "{}{}\x1b[39m",
            fg(palette.subtle),
            super::sidebar_i18n::git_clean()
        ),
        // 1. 【终端侧栏】【去重统计】单文件展开时明细行已显示增删数，标题仅保留文件计数消除上下重影
        "ready" if !collapsed && git.files.len() == 1 => "1".to_string(),
        "ready" => format!("{} {}", git.files.len(), totals(git)),
        "error" => format!(
            "{}{}\x1b[39m",
            fg(palette.removed),
            super::sidebar_i18n::git_error()
        ),
        _ => format!("{}…\x1b[39m", fg(palette.subtle)),
    };
    let mut rows = vec![row(
        heading_with_tone(
            super::sidebar_i18n::changes_heading(),
            &count,
            collapsed || quiet,
            quiet,
            width,
        ),
        Some(SidebarAction::Toggle(SidebarSection::Git)),
    )];
    if collapsed || quiet {
        return rows;
    }
    match git.state.as_str() {
        "ready" => {
            for (index, (status, path)) in git.files.iter().enumerate() {
                rows.push(row(
                    file(git, status, path, width),
                    Some(SidebarAction::GitFile(index)),
                ));
            }
        }
        "error" => rows.push(row(
            item(super::sidebar_i18n::git_unavailable(), width, true),
            None,
        )),
        _ => rows.push(row(
            item(super::sidebar_i18n::git_checking(), width, true),
            None,
        )),
    }
    rows
}

/// 【终端审查】【文件行】两列缩进排版，状态字母单独着色并与右侧增删统计分离。
/// 参数: git 为快照，status 为文件状态，path 为路径，width 为宽度；返回: 文件行。
pub(super) fn file(git: &GitSummary, status: &str, path: &str, width: usize) -> String {
    let palette = SidebarPalette::current();
    let trimmed = path.trim_end_matches('/');
    let name = trimmed.rsplit('/').next().unwrap_or(trimmed);
    // 1. 【终端审查】【目录标识】未跟踪目录保留结尾斜杠，与同名文件区分
    let suffix = if trimmed.len() < path.len() { "/" } else { "" };
    // 2. 【终端审查】【状态着色】新增、删除、重命名和冲突使用状态色，常规修改保持弱化
    let (marker, color) = if status == "UU" {
        ("!", palette.removed)
    } else if status.contains('D') {
        ("D", palette.removed)
    } else if status == "??" || status.contains('A') {
        ("A", palette.added)
    } else if status.contains('R') {
        ("R", palette.accent)
    } else {
        ("M", palette.muted)
    };
    // 3. 【终端审查】【颜色复位】两列缩进后状态字母单独着色，文件名回到条目正文色
    let label = format!(
        "  {}{marker}\x1b[39m {}{suffix}",
        fg(color),
        safe_text(name)
    );
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
