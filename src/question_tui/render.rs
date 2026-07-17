use super::text::{display_inline, editor_view, truncate_width, wrap_display_text};
use super::{QuestionSession, QuestionState, BAR};
use crate::i18n::text as t;
use crate::question::QuestionRequest;
use anyhow::Result;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::queue;
use crossterm::terminal::{self, Clear, ClearType};
use std::io::Write;
use unicode_width::UnicodeWidthStr;

/// 绘制结构化提问面板，并根据焦点位置维护可见区域。
///
/// # 参数
/// - `session`: 当前终端会话
/// - `request`: 结构化提问请求
/// - `state`: 当前选择和编辑状态
///
/// # 返回值
/// 绘制成功时返回空结果，终端写入失败时返回错误
pub(super) fn draw(
    session: &mut QuestionSession,
    request: &QuestionRequest,
    state: &mut QuestionState,
) -> Result<()> {
    session.clear()?;
    let (cols, _) = terminal::size().unwrap_or((80, 24));
    let content_width = (cols as usize).saturating_sub(3).max(1);
    let mut top_lines = Vec::new();
    let mut body_lines = Vec::new();
    let mut footer_lines = Vec::new();
    let mut edit_body_index = None;
    let mut edit_cursor_offset = 0usize;
    let mut edit_cursor_column = 0usize;
    let mut focused_body_index = None;

    if request.needs_review() {
        top_lines.push(tab_line(request, state));
        top_lines.push(String::new());
    }
    if state.on_confirm(request) {
        for (question, selected) in request.questions.iter().zip(&state.answers) {
            let value = if selected.is_empty() {
                format!("\x1b[31m{}\x1b[0m", t("unanswered", "未回答"))
            } else {
                format!(
                    "\x1b[2m{}\x1b[0m",
                    display_inline(&selected.join(t(" / ", "、")))
                )
            };
            body_lines.push(format!("{}: {value}", question.header));
        }
        footer_lines.push(String::new());
        footer_lines.push(format!(
            "\x1b[2m{}\x1b[0m",
            t(
                "Enter submit · Left/Right switch · Esc twice cancel",
                "Enter 提交 · ←/→ 换题 · Esc 两次取消",
            )
        ));
    } else {
        let question = &request.questions[state.tab];
        top_lines.push(format!("\x1b[1m{}\x1b[0m", question.question.trim()));
        top_lines.push(String::new());
        for (index, option) in question.options.iter().enumerate() {
            let picked = state.answers[state.tab].contains(&option.label);
            if state.selected[state.tab] == index {
                focused_body_index = Some(body_lines.len());
            }
            body_lines.extend(option_lines(
                &option.label,
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
                edit_cursor_column = UnicodeWidthStr::width(if question.multiple {
                    "┃ › [ ] "
                } else {
                    "┃ › "
                });
                body_lines.push(editor_option_line(question.multiple, picked, &editor));
            } else {
                let label = if custom.is_empty() {
                    t("Type your own answer", "输入其他答案").to_string()
                } else {
                    format!("{}: {}", t("Custom", "自定义"), display_inline(custom))
                };
                body_lines.extend(option_lines(
                    &label,
                    "",
                    state.selected[state.tab] == index,
                    picked,
                    question.multiple,
                    content_width,
                ));
            }
        }
        footer_lines.push(String::new());
        if state.editing {
            footer_lines.push(format!(
                "\x1b[2m{}\x1b[0m",
                t(
                    "Enter save · Ctrl+J newline · Esc stop editing",
                    "Enter 保存 · Ctrl+J 换行 · Esc 退出编辑"
                )
            ));
        } else {
            let help = if question.multiple {
                t(
                    "Up/Down select · Tab/Space toggle · Enter select/edit · Left/Right switch",
                    "↑/↓ 选择 · Tab/Space 切换 · Enter 选择/编辑 · ←/→ 换题",
                )
            } else {
                t(
                    "Up/Down select · Enter submit · Left/Right switch",
                    "↑/↓ 选择 · Enter 提交 · ←/→ 换题",
                )
            };
            footer_lines.push(format!(
                "\x1b[2m{help} · Esc ×2 {}\x1b[0m",
                t("cancel", "取消")
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

    let max_content_lines = session.panel_lines as usize;
    let layout = panel_layout(
        top_lines.len(),
        body_lines.len(),
        footer_lines.len(),
        max_content_lines,
        focused_body_index,
        state.scroll_starts[state.tab],
    );
    state.scroll_starts[state.tab] = layout.body_start;
    let visible_lines = top_lines
        .iter()
        .skip(layout.top_start)
        .chain(
            body_lines
                .iter()
                .skip(layout.body_start)
                .take(layout.body_capacity),
        )
        .chain(footer_lines.iter().skip(layout.footer_start));
    for (row, line) in visible_lines.enumerate() {
        queue!(
            session.stdout,
            MoveTo(0, session.anchor_y.saturating_add(row as u16)),
            Clear(ClearType::CurrentLine),
            crossterm::style::Print(BAR),
            crossterm::style::Print(" "),
            crossterm::style::Print(truncate_width(line, content_width))
        )?;
    }
    if state.editing {
        if let Some(index) = edit_body_index.filter(|index| {
            *index >= layout.body_start
                && *index < layout.body_start.saturating_add(layout.body_capacity)
        }) {
            let row = layout.top_budget + index - layout.body_start;
            let cursor_x = edit_cursor_column.saturating_add(edit_cursor_offset);
            queue!(
                session.stdout,
                MoveTo(
                    cursor_x.min(cols.saturating_sub(1) as usize) as u16,
                    session.anchor_y.saturating_add(row as u16)
                ),
                Show
            )?;
        } else {
            queue!(session.stdout, Show)?;
        }
    } else {
        queue!(session.stdout, Hide)?;
    }
    session.stdout.flush()?;
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct PanelLayout {
    pub(super) top_start: usize,
    pub(super) top_budget: usize,
    pub(super) body_start: usize,
    pub(super) body_capacity: usize,
    pub(super) footer_start: usize,
}

/// 计算提问面板顶部、主体和底部区域的可见范围。
///
/// # 参数
/// - `top_len`: 顶部行数
/// - `body_len`: 主体行数
/// - `footer_len`: 底部行数
/// - `max_lines`: 面板最大行数
/// - `focused_body_index`: 当前主体焦点行
/// - `current_body_start`: 当前主体起始行
///
/// # 返回值
/// 各区域的可见范围
pub(super) fn panel_layout(
    top_len: usize,
    body_len: usize,
    footer_len: usize,
    max_lines: usize,
    focused_body_index: Option<usize>,
    current_body_start: usize,
) -> PanelLayout {
    let footer_budget = footer_len.min(max_lines);
    let top_budget = top_len.min(max_lines.saturating_sub(footer_budget));
    let body_capacity = max_lines.saturating_sub(top_budget + footer_budget);
    let max_body_start = body_len.saturating_sub(body_capacity);
    let mut body_start = current_body_start.min(max_body_start);
    if body_capacity == 0 {
        body_start = 0;
    } else if let Some(index) = focused_body_index {
        if index < body_start {
            body_start = index;
        } else if index >= body_start.saturating_add(body_capacity) {
            body_start = index
                .saturating_add(1)
                .saturating_sub(body_capacity)
                .min(max_body_start);
        }
    }
    PanelLayout {
        top_start: top_len.saturating_sub(top_budget),
        top_budget,
        body_start,
        body_capacity,
        footer_start: footer_len.saturating_sub(footer_budget),
    }
}

/// 生成问题标签导航行。
///
/// # 参数
/// - `request`: 结构化提问请求
/// - `state`: 当前回答状态
///
/// # 返回值
/// 带终端样式的标签行
fn tab_line(request: &QuestionRequest, state: &QuestionState) -> String {
    let mut parts = Vec::new();
    for (index, question) in request.questions.iter().enumerate() {
        let answered = !state.answers[index].is_empty();
        let label = if answered {
            format!("{} [x]", question.header)
        } else {
            question.header.clone()
        };
        if state.tab == index {
            parts.push(format!("\x1b[7m {label} \x1b[0m"));
        } else {
            parts.push(format!("\x1b[2m{label}\x1b[0m"));
        }
    }
    if request.needs_review() {
        if state.tab == request.questions.len() {
            parts.push(format!("\x1b[7m {} \x1b[0m", t("Review", "确认")));
        } else {
            parts.push(format!("\x1b[2m{}\x1b[0m", t("Review", "确认")));
        }
    }
    parts.join("  ")
}

/// 生成单个选项及其说明的显示行。
///
/// # 参数
/// - `label`: 选项标题
/// - `description`: 选项说明
/// - `active`: 是否为当前焦点
/// - `picked`: 是否已经选择
/// - `multiple`: 是否为多选问题
/// - `content_width`: 可用显示宽度
///
/// # 返回值
/// 带终端样式的选项行
pub(super) fn option_lines(
    label: &str,
    description: &str,
    active: bool,
    picked: bool,
    multiple: bool,
    content_width: usize,
) -> Vec<String> {
    let marker = if multiple {
        if picked {
            "\x1b[35m[x]\x1b[0m "
        } else {
            "\x1b[2m[ ]\x1b[0m "
        }
    } else {
        ""
    };
    let label = if active || picked {
        format!("\x1b[35m{label}\x1b[0m")
    } else {
        label.to_string()
    };
    let pointer = if active { "\x1b[35m>\x1b[0m " } else { "  " };
    let mut lines = vec![format!("{pointer}{marker}{label}")];
    let description = description.trim();
    if !description.is_empty() {
        let indent = if multiple { "      " } else { "  " };
        let width = content_width
            .saturating_sub(UnicodeWidthStr::width(indent))
            .max(1);
        lines.extend(
            wrap_display_text(description, width)
                .into_iter()
                .map(|line| format!("{indent}\x1b[2m{line}\x1b[0m")),
        );
    }
    lines
}

/// 生成自定义答案编辑行。
///
/// # 参数
/// - `multiple`: 是否为多选问题
/// - `picked`: 自定义答案是否已经选择
/// - `editor`: 当前编辑器显示内容
///
/// # 返回值
/// 带终端样式的编辑行
pub(super) fn editor_option_line(multiple: bool, picked: bool, editor: &str) -> String {
    let marker = if multiple {
        if picked {
            "\x1b[35m[x]\x1b[0m "
        } else {
            "\x1b[2m[ ]\x1b[0m "
        }
    } else {
        ""
    };
    let value = if editor.is_empty() {
        format!(
            "\x1b[2m{}\x1b[0m",
            t("Type your own answer", "输入其他答案")
        )
    } else {
        editor.to_string()
    };
    format!("\x1b[35m>\x1b[0m {marker}{value}")
}
