use super::model::Picker;
use crate::config_tui::theme::{
    selection_marks, ACCENT, BOLD, BRAND, DIM, MUTED, RESET, SELECT_BG,
};
use crate::config_tui::ui::{display_width, pad, truncate};
use crate::i18n::text as t;
use std::path::Path;

/// 会话行右侧时间列宽度。
const TIME_COLUMN: usize = 10;

/// 【会话选择】【分组展示】生成工作区标题行与会话行，返回选中行位置。
///
/// 参数:
/// - `picker`: 列表状态
/// - `width`: 行可用显示宽度
///
/// 返回:
/// - 带样式的视觉行与选中行下标
pub(super) fn body(picker: &Picker, width: usize) -> (Vec<String>, usize) {
    let width = width.max(20);
    let mut lines = Vec::new();
    let mut workspace = None;
    let mut selected = 0;
    for (position, index) in picker.visible.iter().enumerate() {
        let target = &picker.targets[*index];
        // 1. 工作区切换时插入分组标题
        if workspace != Some(&target.session.workspace_id) {
            let current = target.session.workspace_id == picker.current_workspace;
            lines.push(group_line(target, current, width));
            workspace = Some(&target.session.workspace_id);
        }
        if position == picker.selected {
            selected = lines.len();
        }
        // 2. 会话行：标题左对齐、相对时间右对齐
        lines.push(session_line(target, position == picker.selected, width));
    }
    if lines.is_empty() {
        lines.push(format!(
            "{MUTED}{}{RESET}",
            t(
                "No matching sessions. Tab switches scope.",
                "没有匹配会话，按 Tab 切换范围。",
            )
        ));
    }
    (lines, selected)
}

/// 生成工作区分组标题：当前目录带标签，路径按家目录缩写。
///
/// 参数:
/// - `target`: 分组首个会话
/// - `current`: 是否为当前工作区
/// - `width`: 行可用显示宽度
///
/// 返回:
/// - 带样式的标题行
fn group_line(target: &crate::state::ResumeTarget, current: bool, width: usize) -> String {
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
        format!("  {BRAND}{}{RESET}", t("current", "当前目录"))
    } else {
        String::new()
    };
    let tag_width = if current {
        2 + display_width(t("current", "当前目录"))
    } else {
        0
    };
    let path = truncate(&path, width.saturating_sub(tag_width).max(8));
    format!("{BOLD}{path}{RESET}{tag}")
}

/// 生成单个会话行，选中行整行深底。
///
/// 参数:
/// - `target`: 会话目标
/// - `selected`: 是否为选中行
/// - `width`: 行可用显示宽度
///
/// 返回:
/// - 带样式的会话行
fn session_line(target: &crate::state::ResumeTarget, selected: bool, width: usize) -> String {
    let (bar, style) = selection_marks(selected);
    let marker = if target.session.is_current {
        "●"
    } else {
        " "
    };
    let time = crate::control_commands::relative_time(&target.session.info.updated_at);
    // 固定占位：缩进 2 + 指示条 1 + 空格 1 + 标记 1 + 空格 1 + 间隔 2 + 时间列
    let title_width = width.saturating_sub(8 + TIME_COLUMN).max(6);
    let title = pad(&target.session.info.title, title_width);
    // 按显示宽度右对齐：格式化宽度按字符计数，中文会多占列
    let time = truncate(&time, TIME_COLUMN);
    let time = format!(
        "{}{time}",
        " ".repeat(TIME_COLUMN.saturating_sub(display_width(&time)))
    );
    if selected {
        format!("  {bar}{style} {marker} {title}  {time}{RESET}")
    } else {
        let marker = if target.session.is_current {
            format!("{BRAND}{marker}{RESET}")
        } else {
            marker.to_string()
        };
        format!("  {bar} {marker} {title}  {MUTED}{time}{RESET}")
    }
}

/// 生成搜索行：范围标签 + 输入内容或占位提示。
///
/// 参数:
/// - `picker`: 列表状态
/// - `width`: 行可用显示宽度
///
/// 返回:
/// - 带样式的搜索行与光标所在列（相对行首）
pub(super) fn search_line(picker: &Picker, width: usize) -> (String, usize) {
    let scope = if picker.all {
        t("All workspaces", "全部工作区")
    } else {
        t("Current workspace", "当前工作区")
    };
    let chip = format!("{SELECT_BG}{ACCENT} {scope} {RESET}");
    let chip_width = display_width(scope) + 2;
    let prefix_width = chip_width + 3;
    let query_budget = width.saturating_sub(prefix_width).max(4);
    let (text, query_width) = if picker.query.is_empty() {
        let hint = t(
            "type to search title, id or path",
            "输入以搜索标题、编号或路径",
        );
        (format!("{DIM}{}{RESET}", truncate(hint, query_budget)), 0)
    } else {
        let query = truncate(&picker.query, query_budget);
        let width = display_width(&query);
        (query, width)
    };
    (
        format!("{chip} {ACCENT}›{RESET} {text}"),
        prefix_width + query_width,
    )
}

/// 生成选中会话的详情两行：编号与更新时间、完整目录。
///
/// 参数:
/// - `picker`: 列表状态
/// - `width`: 行可用显示宽度
///
/// 返回:
/// - 两行详情；无选中项时为空行
pub(super) fn detail_lines(picker: &Picker, width: usize) -> [String; 2] {
    let Some(target) = picker.target() else {
        return [String::new(), String::new()];
    };
    // 1. 编号、本地时间与运行实例
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
    // 2. 完整目录，旧会话缺失路径时提示定位方式
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
        format!("{MUTED}{}{RESET}", truncate(&meta, width)),
        format!("{MUTED}{}{RESET}", truncate(&path, width)),
    ]
}

/// 将 RFC 3339 时间转为本地 `YYYY-MM-DD HH:MM`，无法解析时原样返回。
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

/// 将家目录下的路径缩写为 `~/...`。
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
