use super::{symbols, text::display_inline, QuestionState};
use crate::{i18n::text as t, question::QuestionRequest};
use unicode_width::UnicodeWidthStr;

/// 【终端提问】【题目导航】参数为请求、状态与可用列数，返回以当前题为中心的标签行。
pub(super) fn question_tabs(
    request: &QuestionRequest,
    state: &QuestionState,
    width: usize,
) -> String {
    if state.on_confirm(request) {
        return format!("◆ {}", t("Review answers", "确认回答"));
    }
    let labels: Vec<String> = request
        .questions
        .iter()
        .enumerate()
        .map(|(index, question)| {
            format!(
                "{} {}",
                symbols::progress(index == state.tab, !state.answers[index].is_empty()),
                display_inline(&question.header)
            )
        })
        .collect();
    let mut start = state.tab;
    let mut end = state.tab + 1;
    let mut used = UnicodeWidthStr::width(labels[state.tab].as_str());
    // 1. 【终端提问】【导航裁剪】先保留当前题，再按剩余空间补齐左右标签
    while start > 0 || end < labels.len() {
        let candidate = if start > 0 { start - 1 } else { end };
        let extra = UnicodeWidthStr::width(labels[candidate].as_str()) + 3;
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
    if start > 0 {
        parts.push("‹".to_string());
    }
    for (index, label) in labels.iter().enumerate().take(end).skip(start) {
        let style = if index == state.tab {
            "\x1b[1;36m"
        } else if state.answers[index].is_empty() {
            "\x1b[2m"
        } else {
            "\x1b[36m"
        };
        parts.push(format!("{style}{label}\x1b[0m"));
    }
    if end < labels.len() {
        parts.push("›".to_string());
    }
    parts.join("   ")
}

/// 生成问题标签导航行。
///
/// # 参数
/// - `request`: 结构化提问请求
/// - `state`: 当前回答状态
///
/// # 返回值
/// 带终端样式的标签行
pub(super) fn tab_line(request: &QuestionRequest, state: &QuestionState) -> String {
    let answered = state
        .answers
        .iter()
        .filter(|answers| !answers.is_empty())
        .count();
    if state.on_confirm(request) {
        return format!(
            "\x1b[1m{}\x1b[0m  \x1b[2m{answered}/{} {}\x1b[0m",
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
        "\x1b[36m{} {}/{}\x1b[0m  \x1b[2m{} · {mode} · {requirement}\x1b[0m",
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
