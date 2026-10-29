//! 依据实际生效策略计算占用标尺和触发摘要。

use super::{
    card::{ACCENT, BORDER, MUTED, RESET, TRIGGER, USED},
    draft::format_tokens,
    view::PreviewContext,
};
use crate::{i18n::text as t, state::CompactionBudgetPolicy};

/// 【上下文】【容量标尺】区分当前占用、触发位置和预留，所有比例以实际窗口为准。
/// 参数：上下文、策略和宽度；返回：不超过宽度的 ANSI 标尺。
pub(super) fn bar(
    context: &PreviewContext,
    policy: CompactionBudgetPolicy,
    width: usize,
) -> String {
    if width < 3 {
        return "─".repeat(width);
    }
    let size = width - 2;
    let window = context.window.max(1);
    let trigger = policy.trigger_chars(window);
    let used = context.used.unwrap_or(0).min(window);
    let filled = (used as f64 / window as f64 * size as f64) as usize;
    let mark = ((trigger as f64 / window as f64 * size as f64) as usize).min(size - 1);
    let mut text = String::from("[");
    for index in 0..size {
        let (style, ch) = if index == mark {
            (TRIGGER, '┃')
        } else if index < filled {
            (USED, '█')
        } else if index > mark {
            (TRIGGER, '▓')
        } else {
            (BORDER, '░')
        };
        text.push_str(&format!("{style}{ch}{RESET}"));
    }
    text.push(']');
    text
}

/// 【上下文】【阈值摘要】显示真实占用和触发点，未知占用不冒充零。
/// 参数：上下文和策略；返回：可自动折行的摘要。
pub(super) fn summary(context: &PreviewContext, policy: CompactionBudgetPolicy) -> String {
    let trigger = policy.trigger_chars(context.window);
    let used = context
        .used
        .map(format_tokens)
        .unwrap_or_else(|| "—".into());
    let remaining = context
        .used
        .map(|used| super::card::compact(trigger.saturating_sub(used)))
        .unwrap_or_else(|| "—".into());
    format!("{MUTED}{} {USED}{used}{RESET} · {MUTED}{} {TRIGGER}{} ({:.1}%){RESET} · {MUTED}{} {ACCENT}{remaining}{RESET}",
        t("Used", "已用"), t("Trigger", "触发点"), format_tokens(trigger),
        trigger as f64 / context.window.max(1) as f64 * 100.0, t("Remaining", "距触发"))
}
