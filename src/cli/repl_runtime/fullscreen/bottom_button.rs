//! 全屏正文离开底部时，悬浮在输入框正上方、水平居中的“回到底部 / 有新输出”按钮。
//!
//! 这是唯一的回到底部入口；浮动标题右侧不再重复显示新输出提示。

use super::state::FullscreenState;
use crate::cli::repl_text::visible_width;
use crate::cli::repl_transcript_pager::clip_to_width;
use crate::i18n::text as t;

/// 按钮底色与文字：与标题栏同色系，出现新输出时换成强调色。
const BUTTON_STYLE: &str = "\x1b[48;5;238m\x1b[38;5;252m";
const BUTTON_HOVER_STYLE: &str = "\x1b[48;5;244m\x1b[38;5;255m";
const BUTTON_UNSEEN_STYLE: &str = "\x1b[48;2;42;74;66m\x1b[38;2;94;196;168m";
const BUTTON_UNSEEN_HOVER_STYLE: &str = "\x1b[48;2;58;102;90m\x1b[38;2;160;230;210m";
const RESET: &str = "\x1b[0m";
/// 按钮两侧至少保留的正文列数，过窄时不显示按钮。
const SIDE_MARGIN: usize = 2;

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

/// 【全屏视图】【回到底部】正文不在底部时，把按钮居中叠加到紧贴输入框的正文最后一行。
///
/// 参数:
/// - `line`: 已适配正文宽度的最后一行
/// - `state`: 全屏状态
/// - `width`: 正文列数
/// - `pointer_col`: 鼠标在正文区的列；落在按钮上时提亮
///
/// 返回:
/// - 叠加后的行与按钮所在列范围 `[start, end)`；在底部或宽度不足时原样返回
pub(super) fn overlay(
    line: &str,
    state: &FullscreenState,
    width: usize,
    pointer_col: Option<u16>,
) -> (String, Option<(u16, u16)>) {
    if state.follow {
        return (line.to_string(), None);
    }
    let text = label(state);
    let button_width = visible_width(text);
    if button_width + SIDE_MARGIN * 2 > width {
        return (line.to_string(), None);
    }
    let start = (width - button_width) / 2;
    let end = start + button_width;
    let hovered = pointer_col.is_some_and(|col| {
        let col = usize::from(col);
        col >= start && col < end
    });
    let style = match (state.unseen, hovered) {
        (true, true) => BUTTON_UNSEEN_HOVER_STYLE,
        (true, false) => BUTTON_UNSEEN_STYLE,
        (false, true) => BUTTON_HOVER_STYLE,
        (false, false) => BUTTON_STYLE,
    };
    // 按钮两侧的正文原样保留，行宽保持不变
    let left = clip_to_width(line, start);
    let gap = start.saturating_sub(visible_width(&left));
    let right = skip_columns(line, end);
    let output = format!(
        "{left}{RESET}{}{style}{text}{RESET}{right}",
        " ".repeat(gap)
    );
    (output, Some((start as u16, end as u16)))
}

/// 【全屏视图】【列裁剪】跳过行首若干显示列，返回剩余部分并保留之前生效的样式。
///
/// 参数:
/// - `line`: ANSI 行
/// - `columns`: 要跳过的显示列数
///
/// 返回:
/// - 从指定列开始的 ANSI 文本；宽字符跨界时用空格补齐
fn skip_columns(line: &str, columns: usize) -> String {
    let mut styles = String::new();
    let mut col = 0usize;
    let mut index = 0usize;
    while index < line.len() && col < columns {
        if line.as_bytes()[index] == 0x1b {
            let end = crate::render::terminal_image::escape_sequence_end(line, index)
                .max(index + 1)
                .min(line.len());
            styles.push_str(&line[index..end]);
            index = end;
            continue;
        }
        let Some(ch) = line[index..].chars().next() else {
            break;
        };
        col += unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        index += ch.len_utf8();
    }
    let pad = " ".repeat(col.saturating_sub(columns));
    format!("{styles}{pad}{}", &line[index..])
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
        let (line, cols) = overlay("text", &state(true, false), 40, None);
        assert_eq!(line, "text");
        assert!(cols.is_none());
    }

    /// 验证离开底部时按钮水平居中，两侧正文保留，行宽不变。
    #[test]
    fn shown_centered_when_scrolled_up() {
        let body = format!("{:<40}", "left-side transcript text right-side!");
        let (line, cols) = overlay(&body, &state(false, false), 40, None);
        let (start, end) = cols.unwrap();
        assert_eq!(visible_width(&line), 40);
        let width = visible_width(label(&state(false, false)));
        assert_eq!(usize::from(end - start), width);
        assert_eq!(usize::from(start), (40 - width) / 2);
        let text = plain(&line);
        assert!(text.contains(label(&state(false, false)).trim()));
        assert!(text.starts_with(&body[..usize::from(start)]));
        assert!(text.ends_with(&body[usize::from(end)..]));
        let (unseen, _) = overlay(&body, &state(false, true), 40, None);
        assert!(plain(&unseen).contains(t("New output", "有新输出")));
    }

    /// 验证宽字符跨越按钮右边界时用空格补齐，行宽不变。
    #[test]
    fn wide_characters_at_the_edge_keep_the_width() {
        let body = "中".repeat(20);
        let (line, cols) = overlay(&body, &state(false, false), 40, None);
        assert!(cols.is_some());
        assert_eq!(visible_width(&line), 40);
    }

    /// 验证过窄时不显示按钮。
    #[test]
    fn skipped_when_too_narrow() {
        let (_, cols) = overlay("x", &state(false, false), 8, None);
        assert!(cols.is_none());
    }

    /// 指针落在按钮上时换成更亮的样式，文字不变。
    #[test]
    fn hover_brightens_the_button() {
        let body = format!("{:<40}", "left-side transcript text right-side!");
        let idle = overlay(&body, &state(false, false), 40, None).0;
        let start = overlay(&body, &state(false, false), 40, None).1.unwrap().0;
        let hovered = overlay(&body, &state(false, false), 40, Some(start)).0;
        assert_ne!(idle, hovered);
        assert!(hovered.contains(BUTTON_HOVER_STYLE), "{hovered:?}");
        assert!(!hovered.contains("\x1b[1m"), "{hovered:?}");
        assert_eq!(plain(&idle), plain(&hovered));
    }
}
