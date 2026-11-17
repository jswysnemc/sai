use super::model::Picker;
use crate::config_tui::theme::{BOLD, RESET, SELECTION_BAR};
use crate::config_tui::ui::{display_width, pad, truncate};
use crate::i18n::text as t;
use crate::render::panel_palette::{bg, fg, PanelPalette};
use std::path::Path;

/// 会话行右侧时间列宽度。
const TIME_COLUMN: usize = 10;

/// 【会话选择】【分组展示】按当前调色板生成工作区标题行与会话行，返回选中视觉行位置。
///
/// 参数:
/// - `picker`: 列表状态
/// - `width`: 行可用显示宽度
///
/// 返回:
/// - `(带样式的视觉行, 选中行下标, 可点击行映射)`
#[cfg(test)]
pub(super) fn body(picker: &Picker, width: usize) -> (Vec<String>, usize, Vec<(usize, usize)>) {
    body_with_palette(picker, width, PanelPalette::current())
}

/// 【会话选择】【指定主题分组展示】按给定调色板生成工作区标题行与会话行。
///
/// 参数:
/// - `picker`: 列表状态
/// - `width`: 行可用显示宽度
/// - `palette`: 明暗自适应调色板
///
/// 返回:
/// - `(带样式的视觉行, 选中行下标, 可点击行映射)`
pub(super) fn body_with_palette(
    picker: &Picker,
    width: usize,
    palette: PanelPalette,
) -> (Vec<String>, usize, Vec<(usize, usize)>) {
    let width = width.max(20);
    let muted = fg(palette.muted);
    let mut lines = Vec::new();
    let mut targets = Vec::new();
    let mut workspace = None;
    let mut selected = 0;
    for (position, index) in picker.visible.iter().enumerate() {
        let target = &picker.targets[*index];
        // 1. 【会话选择】【工作区分组】切换工作区时插入分组标题
        if workspace != Some(&target.session.workspace_id) {
            let current = target.session.workspace_id == picker.current_workspace;
            lines.push(group_line(target, current, width, palette));
            workspace = Some(&target.session.workspace_id);
        }
        if position == picker.selected {
            selected = lines.len();
        }
        // 2. 【会话选择】【会话行绘制】标题左对齐、相对时间右对齐，选中行保留左侧异色竖条锚点
        targets.push((lines.len(), position));
        lines.push(session_line(
            target,
            position == picker.selected,
            width,
            palette,
        ));
    }
    if picker.all && picker.loading {
        lines.push(format!(
            "{muted}{}{RESET}",
            t(
                "Loading workspaces... Esc cancels.",
                "正在加载工作区，按 Esc 取消。"
            )
        ));
    } else if picker.all && picker.load_error.is_some() {
        lines.push(format!(
            "{muted}{}: {}{RESET}",
            t("Failed to load workspaces", "工作区加载失败"),
            picker.load_error.as_deref().unwrap_or_default()
        ));
    } else if lines.is_empty() {
        lines.push(format!(
            "{muted}{}{RESET}",
            t(
                "No matching sessions. Tab switches scope.",
                "没有匹配会话，按 Tab 切换范围。",
            )
        ));
    }
    (lines, selected, targets)
}

/// 【会话选择】【分组标题】生成工作区分组标题，当前目录附带高亮标签。
///
/// 参数:
/// - `target`: 分组首个会话
/// - `current`: 是否为当前工作区
/// - `width`: 行可用显示宽度
/// - `palette`: 面板调色板
///
/// 返回:
/// - 带样式的分组标题行
fn group_line(
    target: &crate::state::ResumeTarget,
    current: bool,
    width: usize,
    palette: PanelPalette,
) -> String {
    let path = target
        .workspace_path
        .as_ref()
        .map(|path| home_relative(path))
        .unwrap_or_else(|| {
            format!(
                "{} ({})",
                t("Path unavailable", "路径未记录"),
                target.session.workspace_id
            )
        });
    let tag = if current {
        format!("  {}{}{RESET}", fg(palette.accent), t("current", "当前目录"))
    } else {
        String::new()
    };
    let tag_width = if current {
        2 + display_width(t("current", "当前目录"))
    } else {
        0
    };
    let path = truncate_head(&path, width.saturating_sub(tag_width).max(8));
    format!("{BOLD}{}{path}{RESET}{tag}", fg(palette.text))
}

/// 【会话选择】【会话行】生成单个会话行，选中行使用自适应焦点底色并保留左侧 `▏` 竖条锚点。
///
/// 参数:
/// - `target`: 会话目标
/// - `selected`: 是否为选中行
/// - `width`: 行可用显示宽度
/// - `palette`: 面板调色板
///
/// 返回:
/// - 带自适应样式的会话行
pub(super) fn session_line(
    target: &crate::state::ResumeTarget,
    selected: bool,
    width: usize,
    palette: PanelPalette,
) -> String {
    let marker = if target.session.is_current {
        "●"
    } else {
        " "
    };
    let time = crate::control_commands::relative_time(&target.session.info.updated_at);
    let title_width = width.saturating_sub(8 + TIME_COLUMN).max(6);
    let title = pad(&target.session.info.title, title_width);
    let time = truncate(&time, TIME_COLUMN);
    let time = format!(
        "{}{time}",
        " ".repeat(TIME_COLUMN.saturating_sub(display_width(&time)))
    );
    if selected {
        let focus_bg = bg(palette.focus_bg);
        let accent = fg(palette.accent);
        let text_fg = fg(palette.text);
        let muted = fg(palette.muted);
        let marker_styled = if target.session.is_current {
            format!("{focus_bg}{accent}{marker}")
        } else {
            format!("{focus_bg}{text_fg}{marker}")
        };
        let bar = format!("{focus_bg}{accent}{BOLD}{SELECTION_BAR}{RESET}");
        format!(
            "  {bar}{focus_bg} {marker_styled}{focus_bg}{BOLD}{text_fg} {title}  {RESET}{focus_bg}{muted}{time}{RESET}"
        )
    } else {
        let text_fg = fg(palette.text);
        let muted = fg(palette.muted);
        let marker_styled = if target.session.is_current {
            format!("{}{marker}{RESET}", fg(palette.accent))
        } else {
            marker.to_string()
        };
        format!("    {marker_styled} {text_fg}{title}{RESET}  {muted}{time}{RESET}")
    }
}

/// 【会话选择】【搜索行】生成范围胶囊、查询输入与光标列位置。
///
/// 参数:
/// - `picker`: 列表状态
/// - `width`: 行可用显示宽度
/// - `palette`: 面板调色板
///
/// 返回:
/// - `(带样式的搜索行, 光标所在相对列)`
pub(super) fn search_line_with_palette(
    picker: &Picker,
    width: usize,
    palette: PanelPalette,
) -> (String, usize) {
    let scope = if picker.all {
        t("All workspaces", "全部工作区")
    } else {
        t("Current workspace", "当前工作区")
    };
    let accent = fg(palette.accent);
    let muted = fg(palette.muted);
    let text_fg = fg(palette.text);
    let chip = format!("{}{accent} {scope} {RESET}", bg(palette.focus_bg));
    let chip_width = display_width(scope) + 2;
    let prefix_width = chip_width + 3;
    let query_budget = width.saturating_sub(prefix_width).max(4);
    let (text, query_width) = if picker.query.is_empty() {
        let hint = t(
            "type to search title, id or path",
            "输入以搜索标题、编号或路径",
        );
        (format!("{muted}{}{RESET}", truncate(hint, query_budget)), 0)
    } else {
        // 1. 【会话选择】【尾部跟随】超长查询词保留末尾以确保中文输入法光标始终紧跟当前输入字符
        let query = truncate_head(&picker.query, query_budget);
        let q_width = display_width(&query);
        (format!("{BOLD}{text_fg}{query}{RESET}"), q_width)
    };
    (
        format!("{chip} {accent}›{RESET} {text}"),
        prefix_width + query_width,
    )
}

/// 【会话选择】【详情行】生成选中会话的编号、更新时间与完整目录两行详情。
///
/// 参数:
/// - `picker`: 列表状态
/// - `width`: 行可用显示宽度
/// - `palette`: 面板调色板
///
/// 返回:
/// - 两行详情字符串
pub(super) fn detail_lines_with_palette(
    picker: &Picker,
    width: usize,
    palette: PanelPalette,
) -> [String; 2] {
    let Some(target) = picker.target() else {
        return [String::new(), String::new()];
    };
    let muted = fg(palette.muted);
    // 1. 【会话选择】【实例状态】组装编号、本地时间与活跃运行实例
    let instances = crate::runner::session_instances(&target.session.state_dir);
    let mut meta = format!(
        "{} {}  {} {}",
        t("ID", "编号"),
        target.session.info.id,
        t("Updated", "更新"),
        local_time(&target.session.info.updated_at)
    );
    if let Some(record) = instances.first() {
        meta.push_str(&format!("  {} {}", t("Running", "运行中"), record.owner));
    }
    // 2. 【会话选择】【目录展示】展示完整目录，旧会话缺失路径时提示定位操作
    let path = target
        .workspace_path
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| {
            t(
                "Directory unknown, Enter to locate it",
                "目录未记录，按 Enter 定位",
            )
            .into()
        });
    [
        format!("{muted}{}{RESET}", truncate(&meta, width)),
        format!("{muted}{}{RESET}", truncate(&path, width)),
    ]
}

/// 【会话选择】【头部截断】按显示宽度截掉文本开头，超长时以省略号开头保留末尾。
///
/// 参数:
/// - `value`: 原文
/// - `max`: 最大显示宽度
///
/// 返回:
/// - 不超过最大宽度的文本
pub(super) fn truncate_head(value: &str, max: usize) -> String {
    if display_width(value) <= max {
        return value.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut kept = Vec::new();
    let mut width = 1usize;
    for ch in value.chars().rev() {
        let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if width + ch_width > max {
            break;
        }
        kept.push(ch);
        width += ch_width;
    }
    format!("…{}", kept.into_iter().rev().collect::<String>())
}

/// 【会话选择】【时间格式化】将 RFC 3339 时间转为本地 `YYYY-MM-DD HH:MM`。
///
/// 参数:
/// - `value`: 时间字符串
///
/// 返回:
/// - 展示用本地时间
fn local_time(value: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|time| {
            time.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|_| value.to_string())
}

/// 【会话选择】【目录缩写】将家目录下的路径缩写为 `~/...`。
///
/// 参数:
/// - `path`: 绝对路径
///
/// 返回:
/// - 展示用路径
fn home_relative(path: &Path) -> String {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
    match home.map(std::path::PathBuf::from) {
        Some(home) if path == home => "~".into(),
        Some(home) => path
            .strip_prefix(&home)
            .map(|rest| format!("~/{}", rest.display()))
            .unwrap_or_else(|_| path.display().to_string()),
        None => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::terminal_contrast::{contrast, MIN_TEXT_CONTRAST};

    /// 长路径截掉开头，保留能区分工作区的末级目录。
    #[test]
    fn long_paths_keep_their_tail() {
        let path =
            "/private/var/folders/36/tjdph2t965j8snz9_vkdnw0r0000gn/T/.tmpvG7VP8/workspace-a";
        let clipped = truncate_head(path, 30);
        assert!(clipped.starts_with('…'));
        assert!(clipped.ends_with("workspace-a"));
        assert_eq!(display_width(&clipped), 30);
        assert_eq!(truncate_head("short", 30), "short");
    }

    /// 【会话选择测试】【竖条锚点与明暗适配】选中行保留 `▏` 竖条并在浅色/深色背景满足对比度。
    #[test]
    fn selected_session_line_preserves_left_bar_and_adapts_to_light_theme() {
        let root = tempfile::tempdir().unwrap();
        let canonical = crate::platform::windows_path::canonicalize(root.path()).unwrap();
        let paths = crate::paths::SaiPaths::for_tests(&canonical);
        crate::state::ensure_workspace_session(&paths, &canonical, "s1", "My Session").unwrap();
        let target = crate::state::resume_catalog(&paths, &canonical, false)
            .unwrap()
            .remove(0);

        for bg_rgb in [(18, 20, 24), (250, 250, 252)] {
            let palette = PanelPalette::for_background(bg_rgb);
            let selected = session_line(&target, true, 60, palette);
            let plain_selected =
                crate::cli::repl_text::strip_terminal_control_sequences(&selected);
            assert!(plain_selected.contains(SELECTION_BAR));
            assert!(!selected.contains("190;246;255"));
            assert!(contrast(palette.text, palette.focus_bg) >= 3.8);
            assert!(contrast(palette.text, bg_rgb) >= MIN_TEXT_CONTRAST);

            let unselected = session_line(&target, false, 60, palette);
            let plain_unselected =
                crate::cli::repl_text::strip_terminal_control_sequences(&unselected);
            assert!(!plain_unselected.contains(SELECTION_BAR));
            assert_eq!(
                display_width(&plain_selected),
                display_width(&plain_unselected),
                "选中与未选中行宽度必须一致以防文字左右跳动"
            );
        }
    }
}
