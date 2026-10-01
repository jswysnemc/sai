//! 全屏正文离开底部时，正文区右下角的“回到底部”按钮。

use super::state::FullscreenState;
use crate::cli::repl_text::visible_width;
use crate::cli::repl_transcript_pager::clip_to_width;
use crate::i18n::text as t;

/// 按钮底色与文字：与标题栏同色系，出现新输出时换成强调色。
const BUTTON_STYLE: &str = "\x1b[48;5;238m\x1b[38;5;252m";
const BUTTON_UNSEEN_STYLE: &str = "\x1b[48;2;42;74;66m\x1b[1m\x1b[38;2;94;196;168m";
const RESET: &str = "\x1b[0m";
/// 按钮与正文右边缘的距离。
const RIGHT_MARGIN: usize = 1;

/// 返回按钮文字；有新输出时一并提示。
///
/// 参数:
/// - `state`: 全屏状态
///
/// 返回:
/// - 按钮文字（含两侧空格）
fn label(state: &FullscreenState) -> &'static str {
    if state.unseen {
        t(" ↓ New output ", " ↓ 有新输出 ")
    } else {
        t(" ↓ Back to bottom ", " ↓ 回到底部 ")
    }
}

/// 【全屏视图】【回到底部】正文不在底部时，把按钮叠加到正文最后一行的右侧。
///
/// 参数:
/// - `line`: 已适配正文宽度的最后一行
/// - `state`: 全屏状态
/// - `width`: 正文列数
///
/// 返回:
/// - 叠加后的行与按钮所在列范围 `[start, end)`；在底部或宽度不足时原样返回
pub(super) fn overlay(
    line: &str,
    state: &FullscreenState,
    width: usize,
) -> (String, Option<(u16, u16)>) {
    if state.follow {
        return (line.to_string(), None);
    }
    let text = label(state);
    let button_width = visible_width(text);
    if button_width + RIGHT_MARGIN + 4 > width {
        return (line.to_string(), None);
    }
    let start = width - button_width - RIGHT_MARGIN;
    let style = if state.unseen {
        BUTTON_UNSEEN_STYLE
    } else {
        BUTTON_STYLE
    };
    // 左侧正文裁到按钮起点，右侧补齐边距，行宽保持不变
    let left = clip_to_width(line, start);
    let gap = start.saturating_sub(visible_width(&left));
    let output = format!(
        "{left}{}{style}{text}{RESET}{}",
        " ".repeat(gap),
        " ".repeat(RIGHT_MARGIN)
    );
    (output, Some((start as u16, (start + button_width) as u16)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::activity_animation::strip_ansi_for_test as plain;

    /// 构造指定跟随状态的全屏状态。
    fn state(follow: bool, unseen: bool) -> FullscreenState {
        FullscreenState {
            follow,
            unseen,
            ..FullscreenState::default()
        }
    }

    /// 验证停在底部时不显示按钮。
    #[test]
    fn hidden_while_following() {
        let (line, cols) = overlay("text", &state(true, false), 40);
        assert_eq!(line, "text");
        assert!(cols.is_none());
    }

    /// 验证离开底部时按钮贴右显示，行宽不变，列范围与文字一致。
    #[test]
    fn shown_when_scrolled_up() {
        let body = format!("{:<40}", "some long transcript line that fills");
        let (line, cols) = overlay(&body, &state(false, false), 40);
        let (start, end) = cols.unwrap();
        assert_eq!(visible_width(&line), 40);
        let text = plain(&line);
        assert!(text.contains(label(&state(false, false)).trim()));
        assert_eq!(usize::from(end), 40 - RIGHT_MARGIN);
        assert_eq!(usize::from(end - start), visible_width(label(&state(false, false))));
        let (unseen, _) = overlay(&body, &state(false, true), 40);
        assert!(plain(&unseen).contains(t("New output", "有新输出")));
    }

    /// 验证过窄时不显示按钮。
    #[test]
    fn skipped_when_too_narrow() {
        let (_, cols) = overlay("x", &state(false, false), 8);
        assert!(cols.is_none());
    }
}
