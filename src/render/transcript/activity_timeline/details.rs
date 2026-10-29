use super::super::{cell::HistoryCell, line::AnsiLine, tool_cell::ToolCell};
use crate::render::tool_view::{self, ToolView};
use serde_json::Value;

/// 【终端】【节点正文】按步骤类型渲染正文，不再次生成工具标题。
/// 参数: `cell` 为步骤，`width` 为正文宽度，`title_width` 为摘要宽度；返回: 完整物理行。
pub(super) fn render(cell: &HistoryCell, width: usize, title_width: usize) -> Vec<AnsiLine> {
    crate::render::render_width::with_render_width(width, || {
        crate::render::render_expand::with_force_expand(|| {
            let body = match cell {
                HistoryCell::Tool(ToolCell::Invocation(view)) => {
                    invocation(view, cell, width, title_width)
                }
                HistoryCell::Diff(cell) => cell.timeline_details(),
                HistoryCell::Tool(ToolCell::Subagent(cell)) => cell.timeline_details(),
                HistoryCell::Tool(ToolCell::CompactionStarted { turn_count, model }) => {
                    format!("{model}\n{turn_count} turns")
                }
                HistoryCell::Tool(ToolCell::CompactionFinished {
                    message,
                    detail,
                    summary,
                    ..
                }) => [message.as_deref(), detail.as_deref(), summary.as_deref()]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join("\n"),
                _ => String::new(),
            };
            let mut lines = super::aligned_wrap::wrap_aligned_block(&body, width);
            super::super::spacing::trim_leading_visual_blanks(&mut lines);
            super::super::spacing::trim_trailing_visual_blanks(&mut lines);
            lines
        })
    })
}

/// 【终端】【工具正文】使用类型化读取和命令结果，并保留审批控件。
/// 参数: `view` 为工具，`cell` 为摘要来源，`width` 为正文宽度，`title_width` 为摘要宽度；返回: ANSI 正文。
fn invocation(view: &ToolView, cell: &HistoryCell, width: usize, title_width: usize) -> String {
    let arguments = serde_json::from_str::<Value>(&view.arguments).unwrap_or_default();
    let title = super::summary::for_cell(cell, title_width, super::node::awaiting_permission(cell));
    let visible_title = crate::render::clip_to_width(&title, title_width, "");
    let mut body = if let Some(preview) = tool_view::render_read_image_preview(view, width) {
        with_missing_path(
            preview,
            tool_view::image_read_path(view)
                .as_deref()
                .unwrap_or_default(),
            &visible_title,
        )
    } else if let Some(page) = view
        .outcome
        .as_ref()
        .filter(|outcome| outcome.ok && view.name == "read_file")
        .and_then(|outcome| tool_view::parse_read_page(&outcome.output))
    {
        let path = if page.path.is_empty() {
            arguments
                .get("path")
                .and_then(Value::as_str)
                .unwrap_or_default()
        } else {
            &page.path
        };
        with_missing_path(
            tool_view::render_read_body(&page, &arguments),
            path,
            &visible_title,
        )
    } else {
        let args = remaining_arguments(&view.arguments, &visible_title);
        let output = view
            .outcome
            .as_ref()
            .map(|outcome| format_payload(&outcome.output))
            .unwrap_or_default();
        let body = if !args.is_empty() && !output.is_empty() {
            format!("\x1b[2m▸ Input:\x1b[0m\n{args}\n\x1b[2m▸ Output:\x1b[0m\n{output}")
        } else if !args.is_empty() {
            format!("\x1b[2m▸ Input:\x1b[0m\n{args}")
        } else {
            output
        };
        body
    };
    if let Some(progress) = view
        .progress
        .as_deref()
        .filter(|text| !text.starts_with("__"))
    {
        append(&mut body, progress);
    }
    body.push_str(&tool_view::render_permission(view.permission.as_ref()));
    body
}

/// 【终端】【参数去重】省略摘要已完整显示的标量参数，裁剪掉的参数仍可查看。
/// 参数: `arguments` 为参数，`title` 为可见摘要；返回: 剩余参数正文。
fn remaining_arguments(arguments: &str, title: &str) -> String {
    let Ok(mut value) = serde_json::from_str::<Value>(arguments) else {
        return arguments.to_string();
    };
    if let Some(fields) = value.as_object_mut() {
        fields.retain(|key, value| {
            !matches!(
                key.as_str(),
                "command" | "path" | "pattern" | "query" | "url" | "description"
            ) || !value
                .as_str()
                .is_some_and(|text| !text.is_empty() && title.contains(text))
        });
        if fields.is_empty() {
            return String::new();
        }
    }
    tool_view::structured_payload::format_value(&value)
}

/// 【终端】【路径完整性】摘要省略目录时在正文中保留完整路径，完整摘要则不重复。
/// 参数: `body` 为正文，`path` 为来源路径，`title` 为可见摘要；返回: 可回看完整路径的正文。
fn with_missing_path(body: String, path: &str, title: &str) -> String {
    if path.is_empty() || title.contains(path) {
        body
    } else {
        format!("File: {path}\n{body}")
    }
}

/// 【终端】【载荷格式】格式化结构化结果，普通文件或模型文本保持原文。
/// 参数: `text` 为载荷；返回: 可读文本。
fn format_payload(text: &str) -> String {
    tool_view::structured_payload::format(text)
}

/// 【终端】【正文拼接】按需添加换行，不制造额外空白行。
/// 参数: `body` 为目标正文，`text` 为追加内容；返回: 无。
fn append(body: &mut String, text: &str) {
    if !text.is_empty() {
        if !body.is_empty() {
            body.push('\n');
        }
        body.push_str(text);
    }
}
