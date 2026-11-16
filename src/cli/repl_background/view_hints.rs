//! 后台任务面板底框提示：按当前状态列出可用按键与可点击列位，空间不足时整段舍弃。

use super::model::TaskManager;
use super::view_list::RibbonItem;
use crate::i18n::text as t;
use crate::render::ansi_clip::ansi_visible_width;
use crate::render::panel_palette::{fg, PanelPalette};
use crossterm::event::KeyCode;
use std::time::Instant;

/// 【后台任务】【页脚提示】确认、执行中与瞬态反馈优先，其余情况只显示当前可用的按键。
///
/// 参数:
/// - `manager`: 面板状态
/// - `width`: 底框可用宽度
/// - `rect_x`: 面板左上角列偏移，用于构造可点击按键范围
/// - `bottom_row`: 底框所在终端行号
///
/// 返回:
/// - `(已着色页脚文本, 可点击按键热区)`
pub(super) fn footer_with_hits(manager: &TaskManager, width: usize) -> (String, Vec<RibbonItem>) {
    let palette = PanelPalette::current();
    if manager.confirm.is_some() {
        return keys(
            &[
                ("Enter/y", t("confirm", "确认"), KeyCode::Enter),
                ("Esc/n", t("cancel", "取消"), KeyCode::Esc),
            ],
            width,
            &palette,
        );
    }
    if manager.busy {
        return (
            format!(
                "{}{}\x1b[0m",
                fg(palette.muted),
                t("Working...", "正在执行操作…")
            ),
            Vec::new(),
        );
    }
    if let Some((text, error, expiry)) = &manager.notice {
        if *error || Instant::now() < *expiry {
            let color = if *error {
                palette.removed
            } else {
                palette.added
            };
            return (format!("{}{text}\x1b[0m", fg(color)), Vec::new());
        }
    }
    if manager.pager {
        return keys(
            &[
                ("Esc", t("back", "返回"), KeyCode::Esc),
                ("Tab", t("stream", "通道"), KeyCode::Tab),
                ("f", t("follow", "跟随"), KeyCode::Char('f')),
                ("↑↓", t("scroll", "滚动"), KeyCode::Down),
                ("PgUp/PgDn", t("page", "翻页"), KeyCode::PageDown),
                ("g/G", t("first/last", "首尾"), KeyCode::Home),
            ],
            width,
            &palette,
        );
    }
    keys(&list_keys(manager), width, &palette)
}

/// 【后台任务】【列表按键】停止仅对运行中任务显示，清理仅在存在已结束记录时显示。
///
/// 参数:
/// - `manager`: 面板状态
///
/// 返回:
/// - `(按键标签, 说明文字, 触发按键)` 有序列表
fn list_keys(manager: &TaskManager) -> Vec<(&'static str, &'static str, KeyCode)> {
    let selected = manager.selected();
    let running = selected.is_some_and(|task| task.status == "running");
    let mut segments = vec![("Esc", t("back", "返回"), KeyCode::Esc)];
    if selected.is_some() {
        segments.push(("Enter", t("logs", "日志"), KeyCode::Enter));
    }
    segments.push(("Tab", t("filter", "筛选"), KeyCode::Tab));
    if running {
        segments.push(("s", t("stop", "停止"), KeyCode::Char('s')));
        segments.push(("f", t("force", "强制"), KeyCode::Char('f')));
    }
    if manager.tasks.iter().any(|task| task.status != "running") {
        segments.push(("x", t("clean", "清理"), KeyCode::Char('x')));
    }
    segments.push(("v", t("stream", "通道"), KeyCode::Char('v')));
    segments.push(("r", t("refresh", "刷新"), KeyCode::Char('r')));
    segments
}

/// 【后台任务】【分段渲染】按键使用主题强调色或正文色，说明与分隔符减淡；放不下时从末尾整段舍弃。
///
/// 参数:
/// - `segments`: 按键、说明与目标键码
/// - `width`: 可用宽度
/// - `palette`: 当前面板色板
///
/// 返回:
/// - `(已着色文本, 相对内容左起点的点击范围)`
fn keys(
    segments: &[(&str, &str, KeyCode)],
    width: usize,
    palette: &PanelPalette,
) -> (String, Vec<RibbonItem>) {
    let separator = " · ";
    let sep_w = ansi_visible_width(separator);
    let mut used = 0;
    let mut output = String::new();
    let mut hits = Vec::new();
    for (index, (key, label, code)) in segments.iter().enumerate() {
        let gap = if index == 0 { 0 } else { sep_w };
        let seg_w = ansi_visible_width(key) + 1 + ansi_visible_width(label);
        if used + gap + seg_w > width {
            break;
        }
        if index > 0 {
            output.push_str(&format!("{}{separator}", fg(palette.border)));
            used += gap;
        }
        let key_color = match code {
            KeyCode::Enter if segments.len() == 2 => palette.accent,
            KeyCode::Char('f') | KeyCode::Char('x') => palette.warning,
            _ => palette.text,
        };
        output.push_str(&format!(
            "{}{key} {}{label}",
            fg(key_color),
            fg(palette.muted)
        ));
        hits.push(RibbonItem {
            start: used,
            end: used + seg_w,
            key: *code,
        });
        used += seg_w;
    }
    if !output.is_empty() {
        output.push_str("\x1b[0m");
    }
    (output, hits)
}
