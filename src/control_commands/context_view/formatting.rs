use super::ContextOverview;
use crate::i18n::text as t;

pub(super) const FREE: (u8, u8, u8) = (129, 131, 137);
pub(super) const RESERVED: (u8, u8, u8) = (91, 96, 107);

/// 【上下文】【数量格式】千和百万保留一位小数，小数值保留整数。
/// 参数: token 数量；返回: 紧凑数量字符串。
pub(super) fn tokens(value: usize) -> String {
    if value >= 1_000_000 {
        format!("{:.1}M", value as f64 / 1_000_000.0)
    } else if value >= 1_000 {
        format!("{:.1}K", value as f64 / 1_000.0)
    } else {
        value.to_string()
    }
}

/// 【上下文】【比例格式】避免零分母，小于千分之一的非零分类保留可读提示。
/// 参数: 数量、窗口和是否取整；返回: 百分比。
pub(super) fn percent(value: usize, window: usize, integer: bool) -> String {
    let percent = value as f64 / window.max(1) as f64 * 100.0;
    if integer {
        format!("{percent:.0}%")
    } else if value > 0 && percent < 0.1 {
        "<0.1%".into()
    } else {
        format!("{percent:.1}%")
    }
}

/// 【上下文】【着色】按调用表面选择是否添加 ANSI 样式。
/// 参数: 着色开关、样式和文本；返回: 显示文本。
pub(super) fn paint(styled: bool, style: &str, text: &str) -> String {
    if styled && !style.is_empty() {
        format!("{style}{text}\x1b[0m")
    } else {
        text.into()
    }
}

/// 【上下文】【色块】生成固定一列的分类色块。
/// 参数: RGB 颜色、字符和着色开关；返回: 色块文本。
pub(super) fn swatch(color: (u8, u8, u8), glyph: char, styled: bool) -> String {
    paint(
        styled,
        &format!("\x1b[38;2;{};{};{}m", color.0, color.1, color.2),
        &glyph.to_string(),
    )
}

/// 【上下文】【分类图例】对齐分类、数量和比例，并区分空闲与压缩预留。
/// 参数: 视图与着色开关；返回: 图例行。
pub(super) fn legend(view: &ContextOverview, styled: bool) -> Vec<String> {
    let (free, reserved) = view.free_and_reserved();
    let mut entries = view
        .categories
        .iter()
        .map(|category| {
            (
                category.label.clone(),
                category.tokens,
                category.color,
                '█',
                percent(category.tokens, view.window, false),
            )
        })
        .collect::<Vec<_>>();
    entries.push((
        t("Free", "空闲").into(),
        free,
        FREE,
        '█',
        percent(free, view.window, true),
    ));
    entries.push((
        t("Compaction reserve", "压缩预留").into(),
        reserved,
        RESERVED,
        '░',
        format!(
            "{} {} {}",
            t("at", "到"),
            percent(view.compact_trigger, view.window, true),
            t("auto-compact", "自动压缩")
        ),
    ));
    let label_width = entries
        .iter()
        .map(|entry| unicode_width::UnicodeWidthStr::width(entry.0.as_str()))
        .max()
        .unwrap_or(0);
    entries
        .into_iter()
        .map(|(label, count, color, glyph, detail)| {
            let padding =
                label_width.saturating_sub(unicode_width::UnicodeWidthStr::width(label.as_str()));
            format!(
                "{} {label}{}  {:>6}  {detail}",
                swatch(color, glyph, styled),
                " ".repeat(padding),
                tokens(count)
            )
        })
        .collect()
}

/// 【上下文】【本轮用量】显示未命中输入、缓存读写和输出，没有数据时不伪造零读数。
/// 参数: 视图；返回: 本轮或最近请求用量说明。
pub(super) fn usage(view: &ContextOverview) -> String {
    let label = if view.usage_is_turn {
        t("This turn", "本轮")
    } else {
        t("Latest request", "最近请求")
    };
    let Some(usage) = &view.usage else {
        return format!("{label}  {}", t("No usage reported", "尚无用量数据"));
    };
    let input = usage
        .prompt_tokens
        .saturating_sub(usage.cache_read_tokens)
        .saturating_sub(usage.cache_write_tokens);
    format!(
        "{label}  {} {} · {} {} ({}) · {} {} · {} {}",
        t("Input", "输入"),
        tokens(input as usize),
        t("Cache hit", "缓存命中"),
        tokens(usage.cache_read_tokens as usize),
        percent(
            usage.cache_read_tokens as usize,
            usage.prompt_tokens as usize,
            true
        ),
        t("Cache write", "写入缓存"),
        tokens(usage.cache_write_tokens as usize),
        t("Output", "输出"),
        tokens(usage.completion_tokens as usize)
    )
}
