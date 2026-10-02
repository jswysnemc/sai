use super::components::{editor_option_line, option_lines, panel_layout, tab_line};
use super::text::{display_inline, editor_view, wrap_display_text};
use super::QuestionState;
use crate::i18n::text as t;
use crate::question::QuestionRequest;

/// 【终端提问】【页面帧】可见行与自定义输入光标，坐标均相对面板。
pub(super) struct QuestionView {
    pub(super) lines: Vec<String>,
    pub(super) cursor: Option<(usize, usize)>,
}

/// 【终端提问】【页面布局】根据问题、状态和终端尺寸生成完整可见帧。
/// @param request 问题集合；state 交互状态；cols 终端列数；max_lines 面板行数
/// @returns 可见行和编辑光标
pub(super) fn compose(
    request: &QuestionRequest,
    state: &mut QuestionState,
    cols: usize,
    max_lines: usize,
) -> QuestionView {
    let card = super::card::CardLayout::new(cols, max_lines);
    let content_width = card.content_width();
    let mut top_lines = Vec::new();
    let mut body_lines = Vec::new();
    let mut footer_lines = Vec::new();
    let mut edit_body_index = None;
    let mut edit_cursor_offset = 0usize;
    let mut edit_cursor_column = 0usize;
    let mut focused_body_index = None;

    top_lines.push(tab_line(request, state));
    if state.on_confirm(request) {
        for (question, selected) in request.questions.iter().zip(&state.answers) {
            let value = if selected.is_empty() {
                if question.required {
                    format!("\x1b[31m{}\x1b[0m", t("unanswered", "未回答"))
                } else {
                    format!("\x1b[2m{}\x1b[0m", t("skipped", "已跳过"))
                }
            } else {
                format!(
                    "\x1b[2m{}\x1b[0m",
                    display_inline(&selected.join(t(" / ", "、")))
                )
            };
            body_lines.push(format!(
                "\x1b[1m{}\x1b[0m",
                display_inline(&question.header)
            ));
            for line in wrap_display_text(
                &display_inline(&selected.join(t(" / ", "、"))),
                content_width.saturating_sub(2).max(1),
            ) {
                body_lines.push(format!("  {line}"));
            }
            if selected.is_empty() {
                body_lines.push(format!("  {value}"));
            }
        }
        footer_lines.push(format!(
            "\x1b[2m{}\x1b[0m",
            t(
                "Enter submit · Up/Down scroll · Left/Right edit",
                "Enter 提交 · ↑/↓ 滚动 · ←/→ 修改",
            )
        ));
    } else {
        let question = &request.questions[state.tab];
        top_lines.extend(
            wrap_display_text(&display_inline(question.question.trim()), content_width)
                .into_iter()
                .map(|line| format!("\x1b[1m{line}\x1b[0m")),
        );
        top_lines.push(String::new());
        for (index, option) in question.options.iter().enumerate() {
            let picked = state.answers[state.tab]
                .iter()
                .any(|answer| answer == option.answer_value());
            if state.selected[state.tab] == index {
                focused_body_index = Some(body_lines.len());
            }
            body_lines.extend(option_lines(
                &format!("{}. {}", index + 1, option.label),
                &option.description,
                state.selected[state.tab] == index,
                picked,
                question.multiple,
                content_width,
            ));
        }
        if question.custom {
            let index = question.options.len();
            let custom = &state.custom_answers[state.tab];
            let picked = !custom.is_empty() && state.answers[state.tab].contains(custom);
            if state.selected[state.tab] == index {
                focused_body_index = Some(body_lines.len());
            }
            if state.editing && state.selected[state.tab] == index {
                edit_body_index = Some(body_lines.len());
                let editor_prefix_width = if question.multiple { 6 } else { 2 };
                let (editor, cursor_offset) = editor_view(
                    &state.edit_buffer,
                    state.edit_cursor,
                    content_width.saturating_sub(editor_prefix_width),
                );
                edit_cursor_offset = cursor_offset;
                edit_cursor_column = 2 + editor_prefix_width;
                body_lines.push(editor_option_line(question.multiple, picked, &editor));
            } else {
                let label = if custom.is_empty() {
                    t("Type your own answer", "输入其他答案").to_string()
                } else {
                    format!("{}: {}", t("Custom", "自定义"), display_inline(custom))
                };
                body_lines.extend(option_lines(
                    &format!("{}. {label}", index + 1),
                    "",
                    state.selected[state.tab] == index,
                    picked,
                    question.multiple,
                    content_width,
                ));
            }
        }
        if state.editing {
            footer_lines.push(format!(
                "\x1b[2m{}\x1b[0m",
                t("Enter save · Esc back", "Enter 保存 · Esc 返回")
            ));
            footer_lines.push(format!(
                "\x1b[2m{}\x1b[0m",
                t("Ctrl+J newline", "Ctrl+J 换行")
            ));
        } else {
            let help = if question.multiple {
                if content_width < 48 {
                    t(
                        "Space toggle · Enter next/edit",
                        "空格勾选 · Enter 继续/编辑",
                    )
                } else {
                    t(
                        "Up/Down focus · Space toggle · Enter next/edit",
                        "↑/↓ 移动 · 空格勾选 · Enter 继续/编辑",
                    )
                }
            } else if content_width < 48 {
                t("Up/Down · Enter choose/edit", "↑/↓ 选择 · Enter 确认/编辑")
            } else {
                t(
                    "Up/Down focus · Enter choose/edit",
                    "↑/↓ 移动 · Enter 确认/编辑",
                )
            };
            footer_lines.push(format!("\x1b[2m{help}\x1b[0m"));
            footer_lines.push(format!(
                "\x1b[2m{}\x1b[0m",
                t(
                    "1–9 choose · Tab next · Esc twice cancel",
                    "1–9 选择 · Tab 换题 · Esc 两次取消"
                )
            ));
        }
    }

    if state.cancel_armed_until.is_some() {
        footer_lines.push(format!(
            "\x1b[1m\x1b[33m{}\x1b[0m",
            t(
                "Press Esc again to cancel this response",
                "再次按 Esc 取消本轮回复"
            )
        ));
    }

    let heading = if card.framed {
        Some(top_lines.remove(0))
    } else {
        None
    };
    let max_content_lines = max_lines.saturating_sub(if card.framed { 3 } else { 0 });
    let layout = panel_layout(
        top_lines.len(),
        body_lines.len(),
        footer_lines.len(),
        max_content_lines,
        focused_body_index,
        state.scroll_starts[state.tab],
    );
    state.scroll_starts[state.tab] = layout.body_start;

    let mut lines = Vec::new();
    if let Some(title) = &heading {
        lines.push(card.heading(title));
    }
    lines.extend(
        top_lines
            .iter()
            .skip(layout.top_start)
            .take(layout.top_budget)
            .chain(
                body_lines
                    .iter()
                    .skip(layout.body_start)
                    .take(layout.body_capacity),
            )
            .map(|line| card.row(line)),
    );
    if card.framed {
        lines.push(card.rule(false));
    }
    lines.extend(
        footer_lines
            .iter()
            .skip(layout.footer_start)
            .map(|line| card.row(line)),
    );
    if card.framed {
        lines.push(card.rule(true));
    }
    lines.resize(max_lines, String::new());
    let cursor = edit_body_index
        .filter(|index| {
            *index >= layout.body_start && *index < layout.body_start + layout.body_capacity
        })
        .map(|index| {
            (
                (edit_cursor_column + edit_cursor_offset).min(cols.saturating_sub(1)),
                layout.top_budget + index - layout.body_start + usize::from(card.framed),
            )
        });
    QuestionView { lines, cursor }
}
