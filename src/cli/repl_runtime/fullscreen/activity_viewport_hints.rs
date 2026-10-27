use crate::render::terminal_text as t;
use crate::render::transcript::AnsiLine;

/// 【终端】【链内提示】按可用宽度优先显示位置和恢复跟随入口，不重复长操作说明。
/// 参数: `first`、`last` 为可见源行，`count` 为总行数，`follow` 为跟随状态，`width` 为净宽；返回: 提示行。
#[cfg(test)]
pub(super) fn footer(
    first: usize,
    last: usize,
    count: usize,
    follow: bool,
    width: usize,
) -> AnsiLine {
    footer_with_focus(first, last, count, follow, width, false)
}

/// 【终端】【带焦点链内提示】当视口被键盘选中时突出焦点标记。
///
/// 参数:
/// - `first`: 起始可见行
/// - `last`: 结束可见行
/// - `count`: 链内总行数
/// - `follow`: 是否跟随最新
/// - `width`: 净宽
/// - `focused`: 是否当前键盘选中
///
/// 返回:
/// - 提示行 ANSI Line
pub(super) fn footer_with_focus(
    first: usize,
    last: usize,
    count: usize,
    follow: bool,
    width: usize,
    focused: bool,
) -> AnsiLine {
    let position = format!("{first}–{last}/{count}");
    let action = if follow {
        t("Following latest", "跟随最新步骤")
    } else {
        t("Click to follow latest", "点击恢复跟随")
    };
    let compact = if follow {
        t("Latest", "最新")
    } else {
        t("Resume", "恢复跟随")
    };
    let keyboard = t("Alt+PgUp/PgDn", "Alt+PgUp/PgDn");
    let track = crate::render::transcript::deepseek_timeline::progress_bar::render_scroll_track(
        first, last, count, 4,
    );
    let candidates = [
        format!("{position} {track} · {action} · {keyboard}"),
        format!("{position} · {action} · {keyboard}"),
        format!("{position} {track} · {action}"),
        format!("{position} · {action}"),
        format!("{position} · {compact}"),
        position,
    ];
    let budget = width.saturating_sub(if focused { 4 } else { 2 });
    let text = candidates
        .iter()
        .find(|text| unicode_width::UnicodeWidthStr::width(text.as_str()) <= budget)
        .unwrap_or(&candidates[3]);
    let prefix = if focused { "  ● " } else { "  " };
    AnsiLine::new(format!(
        "\x1b[2m{}\x1b[0m",
        crate::render::clip_to_width(&format!("{prefix}{text}"), width, "")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【终端】【提示测试】窄窗口不越界，宽窗口同时显示位置和键盘入口。
    /// 参数: 无；返回: 无。
    #[test]
    fn activity_viewport_hints_fit_width_and_describe_actions() {
        crate::i18n::with_locale(crate::i18n::Locale::En, || {
            for width in [1, 2, 4, 10, 20, 30, 50, 100] {
                let hint = footer(32, 40, 40, false, width);
                let text = crate::render::activity_animation::strip_ansi_for_test(hint.as_str());
                assert!(unicode_width::UnicodeWidthStr::width(text.as_str()) <= width);
            }
            let paused = footer(32, 40, 40, false, 100);
            assert!(paused.as_str().contains("Click to follow latest"));
            assert!(paused.as_str().contains("Alt+PgUp/PgDn"));
            assert!(footer(32, 40, 40, true, 100)
                .as_str()
                .contains("Following latest"));
        });
    }
}
