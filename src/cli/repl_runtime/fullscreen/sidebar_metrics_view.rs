//! 紧凑指标格式与极简主义单轴线型上下文进度条。

use crate::render::panel_palette::{fg, PanelPalette};

/// 已占用段细实横线字形（与未占用轨道严格同轴等高）。
const FILLED_SEGMENT: char = '━';
/// 未占用段轻量轨道字形。
const TRACK_SEGMENT: char = '─';

/// 【终端指标】【数量缩写】参数 value 为 token 数；返回使用 k/M 单位的紧凑数值。
pub(super) fn tokens(value: usize) -> String {
    if value >= 1_000_000 {
        format!("{}M", decimal(value as f64 / 1_000_000.0))
    } else if value >= 1_000 {
        format!("{}k", decimal(value as f64 / 1_000.0))
    } else {
        value.to_string()
    }
}

/// 【终端指标】【小数精度】参数 value 为缩放数；返回最多一位小数，整数不保留尾零。
fn decimal(value: f64) -> String {
    format!("{value:.1}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

/// 【终端指标】【上下文读数】参数 context 为已用量与窗口；返回数值和实际百分比，零窗口视为未知。
pub(super) fn context(context: Option<(usize, usize)>) -> String {
    context.filter(|(_, window)| *window > 0).map_or_else(
        || "-- / -- · --%".into(),
        |(used, window)| {
            format!(
                "{} / {} · {:.1}%",
                tokens(used),
                tokens(window),
                (used as f64 / window as f64 * 1000.0).round() / 10.0
            )
        },
    )
}

/// 【终端指标】【简约上下文条】使用同基线单轴横线表达占用比例，按语义水位平稳切换色调。
/// 参数: context 为已用量与窗口，width 为可用列数；返回: 不超过 width 的简约 ANSI 进度条。
pub(super) fn bar(context: Option<(usize, usize)>, width: usize) -> String {
    let count = width.min(24);
    if count == 0 {
        return String::new();
    }
    let palette = PanelPalette::current();
    let ratio = context
        .filter(|(_, window)| *window > 0)
        .map(|(used, window)| (used as f64 / window as f64).clamp(0.0, 1.0));

    // 1. 【终端指标】【非零保底】存在实际上下文占用时至少点亮一格，零负载与未知窗口保持纯净细轨
    let filled = ratio.map_or(0, |value| {
        if value > 0.0 {
            ((value * count as f64).ceil() as usize).clamp(1, count)
        } else {
            0
        }
    });

    // 2. 【终端指标】【语义单色】常规状态使用单一主题色，仅在 72% 与 90% 阈值切换预警色阶
    let fill_tone = match ratio {
        Some(value) if value >= 0.90 => palette.removed,
        Some(value) if value >= 0.72 => palette.warning,
        _ => palette.accent,
    };

    let filled_part = FILLED_SEGMENT.to_string().repeat(filled);
    let empty_part = TRACK_SEGMENT.to_string().repeat(count - filled);
    if filled == 0 {
        format!("{}{empty_part}\x1b[0m", fg(palette.subtle))
    } else if filled == count {
        format!("{}{filled_part}\x1b[0m", fg(fill_tone))
    } else {
        format!(
            "{}{filled_part}{}{empty_part}\x1b[0m",
            fg(fill_tone),
            fg(palette.subtle)
        )
    }
}
