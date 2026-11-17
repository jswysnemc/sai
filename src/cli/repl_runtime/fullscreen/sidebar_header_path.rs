//! 侧栏顶栏工作区目录与分支双行排版：避让右上角按钮，突出末段仓库名并独立展示分支。

use super::sidebar_palette::{fg, SidebarPalette};
use super::sidebar_style::clip;
use super::sidebar_widgets::{columns, safe_text};
use crate::cli::repl_text::visible_width;
use crate::i18n::text as t;

/// 【终端侧栏】【顶栏正文行数】高屏分两行展示目录与分支，中等矮屏合并为单行，极矮屏隐藏。
/// 参数: rows 为侧栏总高度；返回: 不含顶部下沉空行的顶栏文本行数。
pub(super) fn content_height(rows: usize) -> usize {
    if rows >= 20 {
        2
    } else if rows >= 14 {
        1
    } else {
        0
    }
}

/// 【终端侧栏】【双行顶栏正文】生成目录行与分支行（顶部下沉偏移由纵向布局统一控制）。
/// 参数: directory 为工作区路径，branch 为可选分支名，width 为面板净宽，rows 为侧栏高度；返回: 顶栏正文行。
pub(super) fn body_rows(
    directory: &str,
    branch: Option<&str>,
    width: usize,
    rows: usize,
) -> Vec<String> {
    let count = content_height(rows);
    if count == 0 || width == 0 {
        return vec![String::new(); count];
    }
    // 1. 【终端侧栏】【双行分离】高屏第 1 行独立展示目录路径，第 2 行对齐展示 Git 分支
    if count >= 2 {
        vec![
            format_directory_line(directory, width),
            format_branch_line(branch, width),
        ]
    } else {
        vec![format_compact_line(directory, branch, width)]
    }
}

/// 【终端侧栏】【目录行排版】展示品牌菱形锚点与尾部保真工作区路径，突出末段项目名。
/// 参数: directory 为原始工作区路径，width 为面板净宽；返回: 单行目录 ANSI 文本。
pub(super) fn format_directory_line(directory: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let palette = SidebarPalette::current();
    let clean_dir = safe_text(directory.trim());
    let budget = width.saturating_sub(2);
    let short_dir = compact_directory(&clean_dir, budget);
    // 1. 【终端侧栏】【父级降噪】父路径使用次要色，末段项目目录名加粗提亮，兼顾路径上下文与辨识度
    let styled_dir = style_directory(&short_dir, &palette);
    clip(&format!("{}◆ {styled_dir}\x1b[0m", fg(palette.accent)), width)
}

/// 【终端侧栏】【分支行排版】与上方目录文字左对齐，独立一行展示 Git 分支或本地目录状态。
/// 参数: branch 为可选分支名，width 为面板净宽；返回: 单行分支 ANSI 文本。
pub(super) fn format_branch_line(branch: Option<&str>, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let palette = SidebarPalette::current();
    let clean_branch = safe_text(branch.unwrap_or_default().trim());
    let budget = width.saturating_sub(4);
    if budget == 0 {
        return String::new();
    }
    // 1. 【终端侧栏】【字体自适应】按 Nerd Font 开关选择单列分支图标，非仓库目录展示静默本地标识
    if clean_branch.is_empty() {
        let local = clip(t("local", "本地目录"), budget);
        return format!("  {}· {local}\x1b[0m", fg(palette.subtle));
    }
    let icon = if crate::render::nerd_font::nerd_font_enabled() {
        "\u{e0a0}"
    } else {
        "⊢"
    };
    let fitted = clip(&clean_branch, budget);
    format!(
        "  {}{icon} {}{fitted}\x1b[0m",
        fg(palette.accent),
        fg(palette.muted)
    )
}

/// 【终端侧栏】【矮屏单行回退】高度受限时将目录与分支压缩为一行。
/// 参数: directory 为目录路径，branch 为可选分支，width 为净宽；返回: 单行紧凑 ANSI 文本。
fn format_compact_line(directory: &str, branch: Option<&str>, width: usize) -> String {
    let palette = SidebarPalette::current();
    let clean_dir = safe_text(directory.trim());
    let clean_branch = safe_text(branch.unwrap_or_default().trim());
    let styled_branch = if clean_branch.is_empty() {
        String::new()
    } else {
        let fitted = clip(&clean_branch, (width / 2).max(6));
        format!("{}{fitted}\x1b[39m", fg(palette.muted))
    };
    let right_cols = visible_width(&styled_branch);
    let dir_budget = width.saturating_sub(2 + right_cols + usize::from(right_cols > 0));
    let short_dir = compact_directory(&clean_dir, dir_budget);
    let left = format!(
        "{}◆ {}\x1b[0m",
        fg(palette.accent),
        style_directory(&short_dir, &palette)
    );
    columns(&left, &styled_branch, width)
}

/// 【终端侧栏】【路径分层着色】拆分父目录前缀与末段目录名称，强化当前项目主视觉。
/// 参数: short_dir 为已适配预算的路径，palette 为侧栏色板；返回: 带 ANSI 前景的字符串。
fn style_directory(short_dir: &str, palette: &SidebarPalette) -> String {
    if short_dir.is_empty() {
        return format!("{}\x1b[1msai\x1b[22m", fg(palette.text));
    }
    let trimmed = short_dir.trim_end_matches(['/', '\\']);
    if let Some(pos) = trimmed.rfind(['/', '\\']) {
        let split_at = pos + 1;
        let (parent, leaf) = trimmed.split_at(split_at);
        if !leaf.is_empty() {
            return format!(
                "{}{parent}{}\x1b[1m{leaf}\x1b[22m",
                fg(palette.muted),
                fg(palette.text)
            );
        }
    }
    format!("{}\x1b[1m{short_dir}\x1b[22m", fg(palette.text))
}

/// 【终端侧栏】【目录浓缩】优先保留末尾目录名称，空间不足时不截断成头部残片。
/// 参数: path 为安全化后的路径文本，budget 为目录可用列宽；返回: 适配预算的目录文本，过窄时返回空串。
pub(super) fn compact_directory(path: &str, budget: usize) -> String {
    if path.is_empty() || budget < 2 {
        return String::new();
    }
    if visible_width(path) <= budget {
        return path.to_string();
    }
    let trimmed = path.trim_end_matches(['/', '\\']);
    let leaf = trimmed
        .rsplit(['/', '\\'])
        .find(|segment| !segment.is_empty() && *segment != "~")
        .unwrap_or(trimmed);
    if leaf.is_empty() {
        return clip(path, budget);
    }

    // 1. 【终端侧栏】【层级保留】预算允许时使用省略前缀保留路径语义
    let prefixed = format!("…/{leaf}");
    if prefixed != path && visible_width(&prefixed) <= budget {
        return prefixed;
    }
    // 2. 【终端侧栏】【末段回退】中等窄栏直接显示末段仓库名，超长仓库名再按列宽裁剪
    if visible_width(leaf) <= budget {
        return leaf.to_string();
    }
    if budget >= 4 {
        clip(leaf, budget)
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::ansi_clip::strip_ansi;

    /// 【侧栏顶栏测试】【双行分离排版】验证目录与分支分两行完整展示且窄预算保留末段目录名。
    /// 参数: 无；返回: 无。
    #[test]
    fn body_rows_split_directory_and_branch_across_two_lines() {
        assert_eq!(compact_directory("~/workspace/sai", 20), "~/workspace/sai");
        assert_eq!(compact_directory("~/workspace/sai", 6), "…/sai");
        assert_eq!(compact_directory("~/workspace/sai", 3), "sai");
        assert_eq!(compact_directory("~/workspace/sai", 1), "");

        let rows = body_rows("~/workspace/sai", Some("feat/sidebar-tui-polish"), 35, 32);
        assert_eq!(rows.len(), 2);
        let dir_plain = strip_ansi(&rows[0]);
        let branch_plain = strip_ansi(&rows[1]);
        assert!(dir_plain.contains("~/workspace/sai"), "{dir_plain}");
        assert!(branch_plain.contains("feat/sidebar-tui-polish"), "{branch_plain}");
        assert!(visible_width(&rows[0]) <= 35);
        assert!(visible_width(&rows[1]) <= 35);
    }
}
