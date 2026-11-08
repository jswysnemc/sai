//! 后台任务面板底框提示：按当前状态列出可用按键，空间不足时整段舍弃。

use super::model::TaskManager;
use crate::i18n::text as t;
use crate::render::ansi_clip::ansi_visible_width;
use crate::render::panel_palette::{fg, PanelPalette};
use std::time::Instant;

/// 【后台任务】【页脚提示】确认、执行中与瞬态反馈优先，其余情况只显示当前可用的按键。
/// 参数: manager 为面板状态，width 为底框可用宽度；返回: 已着色的提示文本。
pub(super) fn footer(manager: &TaskManager, width: usize) -> String {
    let palette = PanelPalette::current();
    if manager.confirm.is_some() {
        return keys(
            &[
                ("Enter/y", t("confirm", "确认")),
                ("Esc/n", t("cancel", "取消")),
            ],
            width,
            &palette,
        );
    }
    if manager.busy {
        return format!(
            "{}{}\x1b[0m",
            fg(palette.muted),
            t("Working...", "正在执行操作…")
        );
    }
    if let Some((text, error, expiry)) = &manager.notice {
        if *error || Instant::now() < *expiry {
            let color = if *error {
                palette.removed
            } else {
                palette.added
            };
            return format!("{}{text}\x1b[0m", fg(color));
        }
    }
    if manager.pager {
        return keys(
            &[
                ("Esc", t("back", "返回")),
                ("Tab", t("stream", "通道")),
                ("f", t("follow", "跟随")),
                ("↑↓", t("scroll", "滚动")),
                ("PgUp/PgDn", t("page", "翻页")),
                ("g/G", t("first/last", "首尾")),
            ],
            width,
            &palette,
        );
    }
    keys(&list_keys(manager), width, &palette)
}

/// 【后台任务】【列表按键】停止仅对运行中任务显示，清理仅在存在已结束记录时显示。
/// 参数: manager 为面板状态；返回: 按键与说明的有序列表。
fn list_keys(manager: &TaskManager) -> Vec<(&'static str, &'static str)> {
    let selected = manager.selected();
    let running = selected.is_some_and(|task| task.status == "running");
    let mut segments = vec![("Esc", t("back", "返回"))];
    if selected.is_some() {
        segments.push(("Enter", t("logs", "日志")));
    }
    segments.push(("Tab", t("filter", "筛选")));
    if running {
        segments.push(("s", t("stop", "停止")));
        segments.push(("f", t("force", "强制")));
    }
    if manager.tasks.iter().any(|task| task.status != "running") {
        segments.push(("x", t("clean", "清理")));
    }
    segments.push(("r", t("refresh", "刷新")));
    segments
}

/// 【后台任务】【分段渲染】按键使用正文色，说明与分隔符减淡；放不下时从末尾整段舍弃。
/// 参数: segments 为按键与说明，width 为可用宽度，palette 为主题；返回: 已着色文本。
fn keys(segments: &[(&str, &str)], width: usize, palette: &PanelPalette) -> String {
    let separator = " · ";
    let mut used = 0;
    let mut output = String::new();
    for (index, (key, label)) in segments.iter().enumerate() {
        let gap = if index == 0 {
            0
        } else {
            ansi_visible_width(separator)
        };
        let size = gap + ansi_visible_width(key) + 1 + ansi_visible_width(label);
        // 1. 【后台任务】【分段省略】不在按键名称中间截断，保证剩余提示均可直接使用
        if used + size > width {
            break;
        }
        if index > 0 {
            output.push_str(&format!("{}{separator}", fg(palette.muted)));
        }
        output.push_str(&format!(
            "{}{key} {}{label}",
            fg(palette.text),
            fg(palette.muted)
        ));
        used += size;
    }
    if !output.is_empty() {
        output.push_str("\x1b[0m");
    }
    output
}
