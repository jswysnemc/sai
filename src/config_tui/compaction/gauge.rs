//! 依据实际生效策略计算占用标尺和触发摘要。

use super::{
    card::{CardPalette, RESET},
    draft::format_tokens,
    view::PreviewContext,
};
use crate::{i18n::text as t, state::CompactionBudgetPolicy};

/// 【上下文】【容量标尺】区分当前占用、触发位置和预留，所有比例以实际窗口为准。
///
/// 参数:
/// - `context`: 预览窗口上下文
/// - `policy`: 压缩预算策略
/// - `width`: 标尺可用列宽
///
/// 返回:
/// - 不超过列宽的 ANSI 标尺
pub(super) fn bar(
    context: &PreviewContext,
    policy: CompactionBudgetPolicy,
    width: usize,
) -> String {
    if width < 3 {
        return "─".repeat(width);
    }
    let palette = CardPalette::current();
    let size = width;
    let window = context.window.max(1);
    let trigger = policy.trigger_chars(window);
    let used = context.used.unwrap_or(0).min(window);
    let filled = if used > 0 {
        ((used as f64 / window as f64 * size as f64).ceil() as usize).clamp(1, size)
    } else {
        0
    };
    let mark = ((trigger as f64 / window as f64 * size as f64).round() as usize)
        .min(size.saturating_sub(1));
    // 1. 【上下文】【简约单轴标尺】使用同轴细横线表达已用与空闲比例，仅以单格十字刻度标示触发点
    let mut text = String::new();
    for index in 0..size {
        let (style, ch) = if index == mark {
            (palette.trigger, '┼')
        } else if index < filled {
            let tone = if index > mark {
                palette.danger
            } else {
                palette.used
            };
            (tone, '━')
        } else {
            (palette.border, '─')
        };
        text.push_str(&format!("{style}{ch}{RESET}"));
    }
    text
}

/// 【上下文】【阈值摘要】显示真实占用和触发点，未知占用不冒充零。
///
/// 参数:
/// - `context`: 预览窗口上下文
/// - `policy`: 压缩预算策略
///
/// 返回:
/// - 可自动折行的摘要字符串
pub(super) fn summary(context: &PreviewContext, policy: CompactionBudgetPolicy) -> String {
    let palette = CardPalette::current();
    let trigger = policy.trigger_chars(context.window);
    let used = context
        .used
        .map(format_tokens)
        .unwrap_or_else(|| "—".into());
    let remaining = context
        .used
        .map(|used| super::card::compact(trigger.saturating_sub(used)))
        .unwrap_or_else(|| "—".into());
    format!(
        "{}{} {}{used}{RESET} · {}{} {}{} ({:.1}%){RESET} · {}{} {}{remaining}{RESET}",
        palette.muted,
        t("Used", "已用"),
        palette.used,
        palette.muted,
        t("Trigger", "触发点"),
        palette.trigger,
        format_tokens(trigger),
        trigger as f64 / context.window.max(1) as f64 * 100.0,
        palette.muted,
        t("Remaining", "距触发"),
        palette.accent,
    )
}
