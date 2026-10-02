use super::text::{display_inline, wrap_display_text};
use super::QuestionState;
use crate::i18n::text as t;
use crate::question::QuestionRequest;

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
    // 1. 【终端提问】【窄屏布局】至少保留一行选项或编辑器，再分配标题与操作提示
    let body_reserve = usize::from(body_len > 0).min(max_lines);
    let footer_budget = footer_len.min(max_lines.saturating_sub(body_reserve));
    let top_budget = top_len.min(max_lines.saturating_sub(footer_budget + body_reserve));
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
        top_start: 0,
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
        t("Question", "问题"),
        state.tab + 1,
        request.questions.len(),
        display_inline(&question.header)
    )
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
            "[x]"
        } else {
            "[ ]"
        }
    } else if picked {
        "(x)"
    } else {
        "( )"
    };
    let pointer = if active { ">" } else { " " };
    let width = content_width.saturating_sub(6).max(1);
    let label = display_inline(label);
    let mut lines = Vec::new();
    for (index, part) in wrap_display_text(&label, width).into_iter().enumerate() {
        let prefix = if index == 0 {
            format!("{pointer} {marker} ")
        } else {
            "      ".into()
        };
        let color = if active {
            "\x1b[1;36m"
        } else if picked {
            "\x1b[36m"
        } else {
            ""
        };
        lines.push(super::text::truncate_width(
            &format!("{color}{prefix}{part}\x1b[0m"),
            content_width,
        ));
    }
    // 1. 【终端提问】【选项说明】仅展开当前选项的说明，避免长描述淹没其他选择
    if active && !description.trim().is_empty() {
        lines.extend(
            wrap_display_text(&display_inline(description), width)
                .into_iter()
                .map(|line| {
                    super::text::truncate_width(
                        &format!("      \x1b[2m{line}\x1b[0m"),
                        content_width,
                    )
                }),
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
            "\x1b[36m[x]\x1b[0m "
        } else {
            "\x1b[2m[ ]\x1b[0m "
        }
    } else if picked {
        "\x1b[36m(x)\x1b[0m "
    } else {
        "\x1b[2m( )\x1b[0m "
    };
    let value = if editor.is_empty() {
        format!(
            "\x1b[2m{}\x1b[0m",
            t("Type your own answer", "输入其他答案")
        )
    } else {
        editor.to_string()
    };
    format!("\x1b[36m>\x1b[0m {marker}{value}")
}
