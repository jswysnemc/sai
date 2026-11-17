//! 全屏正文离开底部时，悬浮在输入框正上方、水平居中的“回到底部 / 有新输出”按钮。
//!
//! 这是唯一的回到底部入口；浮动标题右侧不再重复显示新输出提示。

use super::state::FullscreenState;
use crate::cli::repl_text::visible_width;
use crate::cli::repl_transcript_pager::clip_to_width;
use crate::i18n::text as t;

const RESET: &str = "\x1b[0m";
/// 按钮两侧至少保留的正文列数，过窄时不显示按钮。
const SIDE_MARGIN: usize = 2;

/// 【终端】【页脚避让】仅当执行链暂停跟随并显示独立恢复页脚时避让，已完成或跟随中的页脚不遮挡回到底部入口。
///
/// 参数:
/// - `state`: 全屏状态
/// - `body_height`: 正文区高度
///
/// 返回:
/// - 末行是否存在需要独立点击恢复的执行链页脚
pub(super) fn conflicts_with_activity_footer(state: &FullscreenState, body_height: usize) -> bool {
    let row = state.scroll + body_height.saturating_sub(1);
    state
        .document
        .activities
        .iter()
        .any(|region| region.footer == Some(row) && !state.activity.is_following(region.key))
}

/// 【全屏视图】【页脚共存】判断正文末行是否为正在跟随或已结算的执行链页脚。
///
/// 参数:
/// - `state`: 全屏状态
/// - `body_height`: 正文区高度
///
/// 返回:
/// - 末行存在非暂停执行链页脚时返回 `true`
fn has_following_activity_footer(state: &FullscreenState, body_height: usize) -> bool {
    let row = state.scroll + body_height.saturating_sub(1);
    state
        .document
        .activities
        .iter()
        .any(|region| region.footer == Some(row) && state.activity.is_following(region.key))
}

/// 【全屏视图】【胶囊文案】返回悬浮胶囊文字，存在未读输出时同步提示。
///
/// 参数:
/// - `state`: 全屏状态
///
/// 返回:
/// - 带内边距空格的胶囊文案
fn label(state: &FullscreenState) -> &'static str {
    if state.unseen {
        t(" ↓ New output ", " ↓ 有新输出 ")
    } else {
        t(" ↓ Back to bottom ", " ↓ 回到底部 ")
    }
}

/// 【全屏视图】【胶囊定位】计算胶囊在末行的起止显示列，避免与左侧执行链状态混叠。
///
/// 参数:
/// - `state`: 全屏状态
/// - `width`: 正文列数
/// - `body_height`: 正文区高度
///
/// 返回:
/// - 胶囊起止列 `[start, end)`；跟随中或宽度不足时返回 `None`
fn placement(state: &FullscreenState, width: usize, body_height: usize) -> Option<(usize, usize)> {
    if state.follow {
        return None;
    }
    let button_width = visible_width(label(state));
    if button_width + SIDE_MARGIN * 2 > width {
        return None;
    }
    // 1. 【全屏视图】【右靠避让】末行含执行链状态页脚且宽度充裕时靠右停放，保留左侧摘要可读
    let start = if has_following_activity_footer(state, body_height)
        && width >= button_width + SIDE_MARGIN + 24
    {
        width - SIDE_MARGIN - button_width
    } else {
        (width - button_width) / 2
    };
    Some((start, start + button_width))
}

/// 【全屏视图】【回到底部】正文不在底部时，把悬浮胶囊叠加到紧贴输入框的正文末行。
///
/// 参数:
/// - `line`: 已适配正文宽度的最后一行
/// - `state`: 全屏状态
/// - `width`: 正文列数
/// - `pointer_col`: 鼠标在正文区的列；落在胶囊上时提亮
///
/// 返回:
/// - 叠加后的行与胶囊所在列范围 `[start, end)`；在底部或宽度不足时原样返回
#[cfg(test)]
pub(super) fn overlay(
    line: &str,
    state: &FullscreenState,
    width: usize,
    pointer_col: Option<u16>,
) -> (String, Option<(u16, u16)>) {
    overlay_at_row(line, state, width, 1, pointer_col)
}

/// 【全屏视图】【末行胶囊】结合正文高度与页脚状态在末行绘制带微底色和呼吸留白的胶囊按钮。
///
/// 参数:
/// - `line`: 已适配正文宽度的最后一行
/// - `state`: 全屏状态
/// - `width`: 正文列数
/// - `body_height`: 正文区高度
/// - `pointer_col`: 鼠标在正文区的列
///
/// 返回:
/// - 叠加后的行与胶囊热区 `[start, end)`
pub(super) fn overlay_at_row(
    line: &str,
    state: &FullscreenState,
    width: usize,
    body_height: usize,
    pointer_col: Option<u16>,
) -> (String, Option<(u16, u16)>) {
    let Some((start, end)) = placement(state, width, body_height) else {
        return (line.to_string(), None);
    };
    let text = label(state);
    let hovered = pointer_col.is_some_and(|col| (start..end).contains(&usize::from(col)));
    // 1. 【全屏视图】【胶囊配色】使用面板微底色与外侧单列呼吸留白，与末行正文清晰分离
    let palette = super::sidebar_palette::SidebarPalette::current();
    let pill_bg = super::sidebar_palette::bg(if hovered {
        palette.focus_bg
    } else {
        palette.hover_bg
    });
    let pill_fg = super::sidebar_palette::fg(if hovered || state.unseen {
        palette.accent
    } else {
        palette.text
    });
    // 2. 【全屏视图】【边界裁剪】胶囊外侧各留一列空白缓冲，宽字符跨界时自动补齐空格保持行宽恒定
    let clear_start = start.saturating_sub(1);
    let clear_end = (end + 1).min(width);
    let line_width = visible_width(line);
    let left = clip_to_width(line, clear_start);
    let left_gap = start.saturating_sub(visible_width(&left));
    let right_gap = if line_width > end {
        clear_end.saturating_sub(end)
    } else {
        0
    };
    let right = skip_columns(line, clear_end);
    let output = format!(
        "{left}{RESET}{}{pill_bg}{pill_fg}{text}{RESET}{}{right}",
        " ".repeat(left_gap),
        " ".repeat(right_gap),
    );
    (output, Some((start as u16, end as u16)))
}

/// 【全屏视图】【按钮悬停】判断指针是否落在回到底部按钮上。
///
/// 按钮与正文最后一行重叠，悬停时只提亮按钮，不再提亮下面的折叠提示。
///
/// 参数:
/// - `state`: 全屏状态
/// - `width`: 正文列数
/// - `body_height`: 正文区行数
///
/// 返回:
/// - 指针在按钮上时为 true
pub(super) fn is_hovered(state: &FullscreenState, width: usize, body_height: usize) -> bool {
    if state.follow
        || conflicts_with_activity_footer(state, body_height)
        || state.pointer_row != Some(body_height.saturating_sub(1))
    {
        return false;
    }
    let Some((start, end)) = placement(state, width, body_height) else {
        return false;
    };
    state
        .pointer_col
        .is_some_and(|col| (start..end).contains(&usize::from(col)))
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

    /// 验证离开底部时胶囊水平居中、外侧保留单列留白且两侧正文保留、行宽不变。
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
        assert!(text.starts_with(&body[..usize::from(start) - 1]));
        assert!(text.ends_with(&body[usize::from(end) + 1..]));
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

    /// 指针落在胶囊上时切换为焦点微底色与强调前景，纯文本保持一致。
    #[test]
    fn hover_brightens_the_button() {
        let body = format!("{:<40}", "left-side transcript text right-side!");
        let palette = super::super::sidebar_palette::SidebarPalette::current();
        let idle = overlay(&body, &state(false, false), 40, None).0;
        let start = overlay(&body, &state(false, false), 40, None).1.unwrap().0;
        let hovered = overlay(&body, &state(false, false), 40, Some(start)).0;
        assert_ne!(idle, hovered);
        assert!(
            idle.contains(&super::super::sidebar_palette::bg(palette.hover_bg)),
            "{idle:?}"
        );
        assert!(
            hovered.contains(&super::super::sidebar_palette::bg(palette.focus_bg)),
            "{hovered:?}"
        );
        assert!(
            hovered.contains(&super::super::sidebar_palette::fg(palette.accent)),
            "{hovered:?}"
        );
        assert!(!hovered.contains("\x1b[1m"), "{hovered:?}");
        assert_eq!(plain(&idle), plain(&hovered));
    }

    /// 验证非暂停的执行链页脚不会隐藏回到底部胶囊，并在宽屏下靠右停放避开左侧状态。
    #[test]
    fn following_activity_footer_docks_button_right_without_hiding_it() {
        let mut current = state(false, true);
        current.document.activities.push(crate::render::transcript::ActivityRegion {
            key: crate::render::transcript::ParagraphKey {
                cell: 1,
                part: crate::render::transcript::ParagraphPart::ActivityGroup,
            },
            start: 0,
            end: 6,
            footer: Some(5),
            scrollable: true,
            active: false,
        });
        assert!(!conflicts_with_activity_footer(&current, 6));
        let body = format!("{:<80}", "  Ran 4 commands");
        let (line, cols) = overlay_at_row(&body, &current, 80, 6, None);
        let (start, end) = cols.unwrap();
        assert!(usize::from(start) > 40, "start={start}, end={end}");
        assert_eq!(visible_width(&line), 80);
        assert!(plain(&line).contains("Ran 4 commands"));
    }
}
