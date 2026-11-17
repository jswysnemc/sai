use super::mouse::MouseTarget;
use super::state::QuestionState;
use super::{symbols, text::display_inline};
use crate::render::panel_palette::{fg, PanelPalette};
use crate::{i18n::text as t, question::QuestionRequest};
use std::ops::Range;
use unicode_width::UnicodeWidthStr;

/// 【终端提问】【题目导航】生成以当前题为中心的标签行。
///
/// 参数:
/// - `request`: 问题请求
/// - `state`: 当前交互状态
/// - `width`: 可用列数
///
/// 返回:
/// - 带自适应样式的题签导航行
#[cfg(test)]
pub(super) fn question_tabs(
    request: &QuestionRequest,
    state: &QuestionState,
    width: usize,
) -> String {
    question_tabs_with_hits(request, state, width, PanelPalette::current()).0
}

/// 【终端提问】【题签与命中】生成题目导航行并返回每个可点击题签相对内容区起点的显示列范围。
///
/// 参数:
/// - `request`: 问题请求
/// - `state`: 当前交互状态
/// - `width`: 内容区可用列数
/// - `palette`: 当前面板调色板
///
/// 返回:
/// - `(带样式的题签行, 相对内容区起点的点击命中列表)`
pub(super) fn question_tabs_with_hits(
    request: &QuestionRequest,
    state: &QuestionState,
    width: usize,
    palette: PanelPalette,
) -> (String, Vec<(Range<usize>, MouseTarget)>) {
    if state.on_confirm(request) {
        let label = format!("◆ {}", t("Review answers", "确认回答"));
        let w = UnicodeWidthStr::width(label.as_str()).min(width);
        return (
            format!("\x1b[1m{}{label}\x1b[0m", fg(palette.accent)),
            vec![(0..w, MouseTarget::PrevTab)],
        );
    }
    let mut labels: Vec<(String, usize, bool)> = request
        .questions
        .iter()
        .enumerate()
        .map(|(index, question)| {
            let answered = !state.answers[index].is_empty();
            (
                format!(
                    "{} {}",
                    symbols::progress(index == state.tab, answered),
                    display_inline(&question.header)
                ),
                index,
                answered,
            )
        })
        .collect();
    if request.needs_review() {
        let review_index = request.questions.len();
        let all_ready = request
            .questions
            .iter()
            .enumerate()
            .all(|(i, q)| !q.required || !state.answers[i].is_empty());
        labels.push((
            format!(
                "{} {}",
                symbols::progress(false, all_ready),
                t("Review", "确认")
            ),
            review_index,
            all_ready,
        ));
    }

    let mut start = state.tab;
    let mut end = state.tab + 1;
    let mut used = UnicodeWidthStr::width(labels[state.tab].0.as_str());
    // 1. 【终端提问】【导航裁剪】先保留当前题，再按剩余空间向两侧扩展题签
    while start > 0 || end < labels.len() {
        let candidate = if start > 0 { start - 1 } else { end };
        let extra = UnicodeWidthStr::width(labels[candidate].0.as_str()) + 3;
        if used + extra > width.saturating_sub(4) {
            break;
        }
        used += extra;
        if candidate < start {
            start -= 1;
        } else {
            end += 1;
        }
    }

    let mut parts = Vec::new();
    let mut hits = Vec::new();
    let mut cursor_col = 0usize;
    let gap = "   ";
    let gap_width = 3usize;
    if start > 0 {
        parts.push(format!("{}‹\x1b[0m", fg(palette.muted)));
        hits.push((cursor_col..cursor_col + 1, MouseTarget::PrevTab));
        cursor_col += 1 + gap_width;
    }
    for (offset, (label, tab_index, answered)) in
        labels.iter().skip(start).take(end - start).enumerate()
    {
        if offset > 0 {
            cursor_col += gap_width;
        }
        let label_width = UnicodeWidthStr::width(label.as_str());
        if cursor_col < width {
            let hit_end = (cursor_col + label_width).min(width);
            if hit_end > cursor_col {
                hits.push((cursor_col..hit_end, MouseTarget::Tab(*tab_index)));
            }
        }
        cursor_col += label_width;
        let style = if *tab_index == state.tab {
            format!("\x1b[1m{}", fg(palette.accent))
        } else if !*answered {
            fg(palette.muted)
        } else {
            fg(palette.accent)
        };
        parts.push(format!("{style}{label}\x1b[0m"));
    }
    if end < labels.len() {
        cursor_col += gap_width;
        if cursor_col < width {
            hits.push((cursor_col..cursor_col + 1, MouseTarget::NextTab));
        }
        parts.push(format!("{}›\x1b[0m", fg(palette.muted)));
    }
    (parts.join(gap), hits)
}

/// 【终端提问】【顶栏摘要】生成问题进度、题型与必答状态的标题摘要行。
///
/// 参数:
/// - `request`: 结构化提问请求
/// - `state`: 当前回答状态
/// - `palette`: 当前面板调色板
///
/// 返回:
/// - 带自适应终端样式的顶栏文本
pub(super) fn tab_line_with_palette(
    request: &QuestionRequest,
    state: &QuestionState,
    palette: PanelPalette,
) -> String {
    let answered = state
        .answers
        .iter()
        .filter(|answers| !answers.is_empty())
        .count();
    let muted = fg(palette.muted);
    if state.on_confirm(request) {
        return format!(
            "\x1b[1m{}{}\x1b[0m  {muted}{answered}/{} {}\x1b[0m",
            fg(palette.text),
            t("Review answers", "确认回答"),
            request.questions.len(),
            t("answered", "已回答")
        );
    }
    let question = &request.questions[state.tab];
    let mode = if question.multiple {
        t("multiple", "多选")
    } else {
        t("single", "单选")
    };
    let requirement = if question.required {
        t("required", "必答")
    } else {
        t("optional", "可跳过")
    };
    format!(
        "{}{} {}/{}\x1b[0m  {muted}{} · {mode} · {requirement}\x1b[0m",
        fg(palette.accent),
        t("Ask", "提问"),
        state.tab + 1,
        request.questions.len(),
        format!(
            "{answered}/{} {}",
            request.questions.len(),
            t("answered", "已回答")
        )
    )
}
