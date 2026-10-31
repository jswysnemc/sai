//! 无背景的队列摘要、固定高度视口及可点击操作，绘制与命中共享几何。

use super::panel_mouse::PanelAction;
use super::queue_panel::{QueuePanelAction, QueuePanelState};
use super::viewport::TerminalSize;
use super::{QueueInsertAt, QueuedSubmission};
use crate::agent::AgentMode;
use crate::cli::repl_text::visible_width;
use crate::i18n::text as t;
use crate::render::ansi_clip::{clip_ansi_to_width, strip_ansi};
use std::ops::Range;

const TEXT: &str = "\x1b[39m";
const MUTED: &str = "\x1b[38;5;244m";
const STEER: &str = "\x1b[38;5;81m";
const NEXT: &str = "\x1b[38;5;214m";
const RESET: &str = "\x1b[0m";

pub(super) struct QueueRow {
    pub(super) text: String,
    pub(super) hits: Vec<(Range<usize>, PanelAction)>,
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct QueueHit {
    pub(super) row: usize,
    pub(super) columns: Range<usize>,
    pub(super) action: PanelAction,
}

/// 【用户队列】【视口限高】列表最多五行，矮屏按四分之一高度收缩，聊天区不随队列数量缩小。
/// 参数: 无；返回: 可见消息数量。
pub(super) fn page_size() -> usize {
    (usize::from(TerminalSize::current().rows) / 4)
        .saturating_sub(2)
        .clamp(1, 5)
}

/// 【用户队列】【时机名称】对用户显示插话和下轮，不使用内部请求/轮次缩写。
/// 参数: timing 为消费时机；返回: 本地化标签。
pub(super) fn timing_label(timing: QueueInsertAt) -> &'static str {
    match timing {
        QueueInsertAt::Request => t("Steer", "次回插话"),
        QueueInsertAt::Turn => t("Next turn", "下轮接力"),
    }
}

/// 【用户队列】【行裁剪】不插入额外视觉行，用户正文控制序列先清除再压成单行。
/// 参数: text 为文本，width 为净宽；返回: 单行安全预览。
fn preview(text: &str, width: usize) -> String {
    let plain = strip_ansi(text)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    clip_ansi_to_width(
        &plain,
        width,
        if visible_width(&plain) > width {
            "…"
        } else {
            ""
        },
    )
}

/// 【用户队列】【完整视图】展开时仅显示选中项附近的片段，收起时保留统计和最新消息。
/// 参数: state 为管理状态，items 为队列快照，streaming 为运行状态；返回: 定宽行及点击范围。
pub(super) fn render(
    state: &QueuePanelState,
    items: &[QueuedSubmission],
    streaming: bool,
) -> Vec<QueueRow> {
    if items.is_empty() && (!state.active || state.deleted.is_empty()) {
        return Vec::new();
    }
    let terminal = TerminalSize::current();
    render_at(state, items, streaming, usize::from(terminal.cols))
}

/// 【用户队列】【宽度适配】列表和操作使用当前聊天区宽度，窄屏优先保留正文与消费时机。
/// 参数: state/items/streaming 为视图来源，width 为当前列宽；返回: 视图行。
pub(super) fn render_at(
    state: &QueuePanelState,
    items: &[QueuedSubmission],
    streaming: bool,
    width: usize,
) -> Vec<QueueRow> {
    if items.is_empty() && (!state.active || state.deleted.is_empty()) {
        return Vec::new();
    }
    let request = items
        .iter()
        .filter(|item| item.insert_at == QueueInsertAt::Request)
        .count();
    let counts = format!(
        "{STEER}{} {request}{RESET} · {NEXT}{} {}{RESET}",
        timing_label(QueueInsertAt::Request),
        timing_label(QueueInsertAt::Turn),
        items.len() - request
    );
    if !state.active {
        let latest = items.last().map(|item| item.text.as_str()).unwrap_or("");
        let prefix = format!(
            "{TEXT}▸ {} ({}) {counts}  {MUTED}Ctrl+↑/Alt+M {}  ",
            t("Queued", "排队消息"),
            items.len(),
            t("manage", "管理")
        );
        let text = format!(
            "{prefix}{}{RESET}",
            preview(latest, width.saturating_sub(visible_width(&prefix)))
        );
        return vec![row(text, PanelAction::QueueToggle, width)];
    }
    let visible = page_size();
    let selected = state
        .selected_id
        .as_ref()
        .and_then(|id| items.iter().position(|item| &item.id == id))
        .unwrap_or(state.selected)
        .min(items.len().saturating_sub(1));
    let start = state
        .scroll
        .min(items.len().saturating_sub(visible))
        .min(selected)
        .max(selected.saturating_add(1).saturating_sub(visible));
    let end = (start + visible).min(items.len());
    let range = format!(
        "↑{start}  {}-{}/{}  ↓{}",
        start + usize::from(!items.is_empty()),
        end,
        items.len(),
        items.len() - end
    );
    let title = format!(
        "{TEXT}\x1b[1m▾ {} ({}){RESET} {counts}",
        t("Queue manager", "队列管理"),
        items.len()
    );
    let room = width.saturating_sub(visible_width(&range) + 1);
    let title = clip_ansi_to_width(&title, room, "");
    let heading = format!(
        "{title}{}{MUTED}{range}{RESET}",
        " ".repeat(width.saturating_sub(visible_width(&title) + visible_width(&range)))
    );
    let mut rows = vec![row(heading, PanelAction::QueueToggle, width)];
    // 2. 【用户队列】【可见消息】每条只占一行，首项标记表示当前时机组内的优先顺序
    for (index, item) in items.iter().enumerate().take(end).skip(start) {
        let mode = match item.mode {
            AgentMode::Yolo => "yolo",
            AgentMode::Audited => "audit",
            AgentMode::AutoAudit => "auto",
            AgentMode::Plan => "plan",
        };
        let timing = timing_label(item.insert_at);
        let color = if item.insert_at == QueueInsertAt::Request {
            STEER
        } else {
            NEXT
        };
        let first = if index == 0 { t("first", "首发") } else { "" };
        let metadata = if width >= 60 {
            format!("{MUTED}[{mode}] {first}{RESET} ")
        } else {
            String::new()
        };
        let prefix = format!(
            "{TEXT}{} #{:<2} {color}{timing}{RESET} {metadata}",
            if index == selected { "›" } else { " " },
            index + 1
        );
        let content = preview(&item.text, width.saturating_sub(visible_width(&prefix)));
        let text = format!(
            "{}{prefix}{TEXT}{content}{RESET}",
            if index == selected { "\x1b[1m" } else { "" }
        );
        rows.push(row(text, PanelAction::QueueItem(index), width));
    }
    let status = if state.pending_delete {
        t(
            "Delete this message? d confirm · Esc cancel",
            "删除当前消息？再次按 d 确认 · Esc 取消",
        )
    } else {
        state.notice.as_deref().unwrap_or("")
    };
    if !status.is_empty() {
        rows.push(QueueRow {
            text: format!("{NEXT}{}{RESET}", preview(status, width)),
            hits: Vec::new(),
        });
    }
    // 3. 【用户队列】【操作控件】窄屏使用短标签，命中只覆盖真正显示的按钮
    let short = width < 68;
    rows.push(controls(
        &[
            ("↑", QueuePanelAction::MoveUp),
            ("↓", QueuePanelAction::MoveDown),
            (if short { "T" } else { t("T Top", "T 置顶") }, QueuePanelAction::Top),
            (if short { "Tab" } else { t("Tab Timing", "Tab 时机") }, QueuePanelAction::ToggleInsert),
            (if short { "d" } else { t("d Delete", "d 删除") }, QueuePanelAction::Delete),
            (if short { "u" } else { t("u Undo", "u 恢复") }, QueuePanelAction::UndoDelete),
        ],
        width,
    ));
    rows.push(controls(
        &[
            (if short { "Enter" } else { t("Enter Edit", "Enter 编辑") }, QueuePanelAction::Edit),
            (
                if short { "s" } else if streaming {
                    t("s Steer now", "s 立即插话")
                } else {
                    t("s Send now", "s 立即发送")
                },
                QueuePanelAction::SendNow,
            ),
            (if short { "Esc" } else { t("Esc Return", "Esc 返回") }, QueuePanelAction::Exit),
        ],
        width,
    ));
    rows
}

/// 【用户队列】【整行目标】标题和消息行支持整行点击，目标不覆盖右栏。
/// 参数: text 为内容，action 为动作，width 为可用宽度；返回: 裁剪后的行。
fn row(text: String, action: PanelAction, width: usize) -> QueueRow {
    QueueRow {
        text: clip_ansi_to_width(&text, width, ""),
        hits: vec![(0..width, action)],
    }
}

/// 【用户队列】【按钮排版】创建独立点击范围，空间不足时省略按钮而不是保留不可见命中区。
/// 参数: actions 为标签与动作，width 为宽度；返回: 单行按钮。
fn controls(actions: &[(&str, QueuePanelAction)], width: usize) -> QueueRow {
    let mut output = QueueRow {
        text: String::new(),
        hits: Vec::new(),
    };
    for (label, action) in actions {
        let label = format!("[ {label} ]");
        let start = visible_width(&output.text);
        let end = start + usize::from(start > 0) + visible_width(&label);
        if end > width {
            break;
        }
        if start > 0 { output.text.push(' '); }
        let start = start + usize::from(start > 0);
        output.text.push_str(&format!("{MUTED}{label}{RESET}"));
        output
            .hits
            .push((start..end, PanelAction::QueueAction(*action)));
    }
    output
}
