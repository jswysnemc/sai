use super::components::{
    editor_option_line_with_palette, option_lines_with_palette, panel_layout,
};
use super::mouse::{MouseTarget, QuestionHitMap};
use super::state::QuestionState;
use super::text::{display_inline, editor_view, strip_ansi, wrap_display_text};
use crate::i18n::text as t;
use crate::question::QuestionRequest;
use crate::render::panel_palette::{fg, PanelPalette};
use std::ops::Range;
use unicode_width::UnicodeWidthStr;

/// 【终端提问】【页面帧】可见行、自定义输入光标与鼠标命中映射，坐标均相对面板左上角。
pub(super) struct QuestionView {
    pub(super) lines: Vec<String>,
    pub(super) cursor: Option<(usize, usize)>,
    pub(super) hits: QuestionHitMap,
}

/// 【终端提问】【页面布局】根据问题、状态和终端尺寸生成完整可见帧及交互命中表。
///
/// 参数:
/// - `request`: 问题集合
/// - `state`: 交互状态
/// - `cols`: 终端列数
/// - `max_lines`: 面板可用行数
///
/// 返回:
/// - 包含可见行、光标位置与鼠标命中的视图帧
pub(super) fn compose(
    request: &QuestionRequest,
    state: &mut QuestionState,
    cols: usize,
    max_lines: usize,
) -> QuestionView {
    compose_with_palette(request, state, cols, max_lines, PanelPalette::current())
}

/// 【终端提问】【指定主题页面布局】按给定调色板生成完整可见帧及交互命中表。
///
/// 参数:
/// - `request`: 问题集合
/// - `state`: 交互状态
/// - `cols`: 终端列数
/// - `max_lines`: 面板可用行数
/// - `palette`: 面板调色板
///
/// 返回:
/// - 包含可见行、光标位置与鼠标命中的视图帧
pub(super) fn compose_with_palette(
    request: &QuestionRequest,
    state: &mut QuestionState,
    cols: usize,
    max_lines: usize,
    palette: PanelPalette,
) -> QuestionView {
    let card = super::card::CardLayout::with_palette(cols, max_lines, palette);
    let content_width = card.content_width();
    let mut top_lines = Vec::new();
    let mut body_lines = Vec::new();
    let mut body_option_indices: Vec<Option<usize>> = Vec::new();
    let mut footer_lines = Vec::new();
    let mut edit_body_index = None;
    let mut edit_cursor_offset = 0usize;
    let mut edit_cursor_column = 0usize;
    let mut focused_body_index = None;

    let muted = fg(palette.muted);
    let text_fg = fg(palette.text);

    // 1. 【终端提问】【顶部与主体组装】分别构建确认页或问题选项页内容
    top_lines.push(super::navigation::tab_line_with_palette(
        request, state, palette,
    ));
    let (tabs_line, tab_hits) =
        super::navigation::question_tabs_with_hits(request, state, content_width, palette);
    top_lines.push(tabs_line);
    let has_tabs_row = true;
    if state.on_confirm(request) {
        for (question, selected) in request.questions.iter().zip(&state.answers) {
            body_lines.push(format!(
                "\x1b[1m{text_fg}{}\x1b[0m",
                display_inline(&question.header)
            ));
            body_option_indices.push(None);
            if selected.is_empty() {
                let value = if question.required {
                    format!("{}{}\x1b[0m", fg(palette.removed), t("unanswered", "未回答"))
                } else {
                    format!("{muted}{}\x1b[0m", t("skipped", "已跳过"))
                };
                body_lines.push(format!("  {value}"));
                body_option_indices.push(None);
            } else {
                for line in wrap_display_text(
                    &display_inline(&selected.join(t(" / ", "、"))),
                    content_width.saturating_sub(2).max(1),
                ) {
                    body_lines.push(format!("  {text_fg}{line}\x1b[0m"));
                    body_option_indices.push(None);
                }
            }
        }
        footer_lines.push(format!(
            "{muted}{}\x1b[0m",
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
                .map(|line| format!("\x1b[1m{text_fg}{line}\x1b[0m")),
        );
        top_lines.push(String::new());
        for (index, option) in question.options.iter().enumerate() {
            let picked = state.answers[state.tab]
                .iter()
                .any(|answer| answer == option.answer_value());
            if state.selected[state.tab] == index {
                focused_body_index = Some(body_lines.len());
            }
            let rendered = option_lines_with_palette(
                &format!("{}. {}", index + 1, option.label),
                &option.description,
                state.selected[state.tab] == index,
                picked,
                question.multiple,
                content_width,
                palette,
            );
            body_option_indices.extend(std::iter::repeat_n(Some(index), rendered.len()));
            body_lines.extend(rendered);
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
                let editor_prefix_width = super::symbols::OPTION_PREFIX_WIDTH;
                let (editor, cursor_offset) = editor_view(
                    &state.edit_buffer,
                    state.edit_cursor,
                    content_width.saturating_sub(editor_prefix_width),
                );
                edit_cursor_offset = cursor_offset;
                edit_cursor_column = 2 + editor_prefix_width;
                body_lines.push(editor_option_line_with_palette(
                    question.multiple,
                    picked,
                    &editor,
                    palette,
                ));
                body_option_indices.push(Some(index));
            } else {
                let draft_preview = state
                    .edit_drafts
                    .get(state.tab)
                    .and_then(Option::as_deref)
                    .map(str::trim)
                    .filter(|draft| !draft.is_empty());
                let label = if !custom.is_empty() {
                    format!("{}: {}", t("Other", "其他"), display_inline(custom))
                } else if let Some(draft) = draft_preview {
                    format!("{}: {}", t("Other (draft)", "其他（草稿）"), display_inline(draft))
                } else {
                    t("Other…", "其他…").to_string()
                };
                let rendered = option_lines_with_palette(
                    &format!("{}. {label}", index + 1),
                    "",
                    state.selected[state.tab] == index,
                    picked,
                    question.multiple,
                    content_width,
                    palette,
                );
                body_option_indices.extend(std::iter::repeat_n(Some(index), rendered.len()));
                body_lines.extend(rendered);
            }
        }
        if state.editing {
            footer_lines.push(format!(
                "{muted}{}\x1b[0m",
                t("Enter save · Esc back", "Enter 保存 · Esc 返回")
            ));
            footer_lines.push(format!(
                "{muted}{}\x1b[0m",
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
                t(
                    "Space toggle · Enter confirm/edit",
                    "空格选择 · Enter 确认/编辑",
                )
            } else {
                t(
                    "Up/Down focus · Space toggle · Enter confirm/edit",
                    "↑/↓ 移动 · 空格选择 · Enter 确认/编辑",
                )
            };
            footer_lines.push(format!("{muted}{help}\x1b[0m"));
            footer_lines.push(format!(
                "{muted}{}\x1b[0m",
                t(
                    "1–9 choose · Tab next · Esc twice cancel",
                    "1–9 选择 · Tab 换题 · Esc 两次取消"
                )
            ));
        }
    }

    if state.cancel_armed_until.is_some() {
        footer_lines.push(format!(
            "\x1b[1m{}{}\x1b[0m",
            fg(palette.warning),
            t(
                "Press Esc again to cancel this response",
                "再次按 Esc 取消本轮回复"
            )
        ));
    }

    // 2. 【终端提问】【视口裁剪】计算标题、主体与底栏预算，自由滚动模式下不强行拉回焦点
    let heading = if card.framed {
        Some(top_lines.remove(0))
    } else {
        None
    };
    let tabs_top_index = has_tabs_row.then_some(if card.framed { 0 } else { 1 });
    let max_content_lines = max_lines.saturating_sub(if card.framed { 3 } else { 0 });
    let effective_focus = if state.free_scroll {
        None
    } else {
        focused_body_index
    };
    let layout = panel_layout(
        top_lines.len(),
        body_lines.len(),
        footer_lines.len(),
        max_content_lines,
        effective_focus,
        state.scroll_starts[state.tab],
    );
    state.scroll_starts[state.tab] = layout.body_start;

    // 3. 【终端提问】【命中映射】将题签、可见选项与确认页操作映射为相对面板坐标
    let mut hits = QuestionHitMap {
        card_width: card.width,
        panel_rows: max_lines,
        max_body_scroll: body_lines.len().saturating_sub(layout.body_capacity),
        regions: Vec::new(),
    };
    let content_left_col = 2usize;
    if let Some(top_idx) = tabs_top_index.filter(|idx| *idx < layout.top_budget) {
        let panel_row = usize::from(card.framed) + top_idx;
        if panel_row < max_lines {
            for (range, target) in tab_hits {
                let start = (content_left_col + range.start).min(card.width);
                let end = (content_left_col + range.end).min(card.width);
                if end > start {
                    hits.regions.push((panel_row, start..end, target));
                }
            }
        }
    }
    let body_rendered = body_lines
        .len()
        .saturating_sub(layout.body_start)
        .min(layout.body_capacity);
    for offset in 0..body_rendered {
        let body_idx = layout.body_start + offset;
        let panel_row = usize::from(card.framed) + layout.top_budget + offset;
        if panel_row >= max_lines {
            break;
        }
        if let Some(Some(opt_idx)) = body_option_indices.get(body_idx) {
            hits.regions
                .push((panel_row, 0..card.width, MouseTarget::Option(*opt_idx)));
        }
    }

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
    let footer_panel_start = lines.len();
    if state.on_confirm(request) && layout.footer_start == 0 && footer_panel_start < max_lines {
        if let Some(first_footer) = footer_lines.first() {
            let plain = strip_ansi(first_footer);
            let submit_phrase = t("Enter submit", "Enter 提交");
            if let Some(range) = phrase_columns(&plain, submit_phrase, content_left_col, card.width)
            {
                hits.regions
                    .push((footer_panel_start, range, MouseTarget::ConfirmSubmit));
            }
            let edit_phrase = t("Left/Right edit", "←/→ 修改");
            if let Some(range) = phrase_columns(&plain, edit_phrase, content_left_col, card.width) {
                hits.regions
                    .push((footer_panel_start, range, MouseTarget::PrevTab));
            }
        }
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
    QuestionView {
        lines,
        cursor,
        hits,
    }
}

/// 【终端提问】【短语定位】计算底栏纯文本中操作短语在面板中的显示列区间。
///
/// 参数:
/// - `plain`: 剥离转义后的单行文本
/// - `phrase`: 待定位的操作短语
/// - `left_offset`: 内容区相对面板左边缘的列偏移
/// - `max_col`: 卡片允许的最大列边界
///
/// 返回:
/// - 落在卡片边界内的列范围
fn phrase_columns(
    plain: &str,
    phrase: &str,
    left_offset: usize,
    max_col: usize,
) -> Option<Range<usize>> {
    let byte_idx = plain.find(phrase)?;
    let start = left_offset + UnicodeWidthStr::width(&plain[..byte_idx]);
    let end = (start + UnicodeWidthStr::width(phrase)).min(max_col);
    (end > start).then_some(start..end)
}
