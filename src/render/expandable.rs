use crate::render::code_block::highlight_code_line;
use crate::render::command_result_block::command_result_streams;
use crate::render::markdown::MarkdownStreamRenderer;
use crate::render::terminal_text as t;
use serde_json::Value;

/// 可在 Ctrl+O 视图中展示的正文类型。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExpandableBlockKind {
    /// Markdown 正文，包含思考内容和代码块。
    Markdown,
    /// 命令工具输出，可能包含 JSON 结果封装。
    Command,
    /// 未指定格式的纯文本。
    Plain,
}

/// 可在 pager 中展开的正文块。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ExpandableBlock {
    /// 标题（含类型与对象名）。
    pub(crate) title: String,
    /// 未渲染的正文。
    pub(crate) body: String,
    /// 正文的渲染类型。
    pub(crate) kind: ExpandableBlockKind,
}

/// 将 Ctrl+O 正文转换为带 ANSI 样式的终端文本。
///
/// 参数:
/// - `kind`: 正文类型
/// - `body`: 未渲染正文
///
/// 返回:
/// - 可直接写入终端的 ANSI 文本
pub(crate) fn render_expandable_body(kind: ExpandableBlockKind, body: &str) -> String {
    match kind {
        ExpandableBlockKind::Markdown => render_markdown_body(body),
        ExpandableBlockKind::Command => render_command_body(body),
        ExpandableBlockKind::Plain => body.to_string(),
    }
}

/// 以稳定 Markdown 渲染器重放正文，使代码围栏和行内语法保持一致。
///
/// 参数:
/// - `body`: Markdown 源文本
///
/// 返回:
/// - ANSI Markdown 文本
fn render_markdown_body(body: &str) -> String {
    if body.is_empty() {
        return String::new();
    }
    let mut renderer = MarkdownStreamRenderer::new_stable();
    let mut rendered = renderer.push(body);
    rendered.push_str(&renderer.flush());
    rendered.trim_end_matches('\n').to_string()
}

/// 将命令结果转换为 stdout/stderr 内容，再逐行进行代码着色。
///
/// 参数:
/// - `body`: 实时 stdout/stderr 或命令工具结果 JSON
///
/// 返回:
/// - 不包含 JSON 结果封装的 ANSI 文本
fn render_command_body(body: &str) -> String {
    let sections = command_sections(body);
    if sections.is_empty() {
        return String::new();
    }

    let mut output = String::new();
    for (index, (label, content)) in sections.into_iter().enumerate() {
        if index > 0 {
            output.push('\n');
        }
        output.push_str("\x1b[1m\x1b[36m── ");
        output.push_str(&label);
        output.push_str(" ──\x1b[0m\n");
        output.push_str(&highlight_output(
            &content,
            label.eq_ignore_ascii_case("stderr") || label == "标准错误",
        ));
    }
    output.trim_end_matches('\n').to_string()
}

/// 从命令结果正文中提取输出流。
///
/// 参数:
/// - `body`: 原始命令正文
///
/// 返回:
/// - `(标签, 内容)` 列表，顺序保持 stdout 后 stderr
fn command_sections(body: &str) -> Vec<(String, String)> {
    if let Some((success, stdout, stderr)) = command_result_streams(body.trim()) {
        let mut sections = Vec::new();
        if !stdout.trim().is_empty() {
            sections.push((t("stdout", "标准输出").to_string(), stdout));
        }
        if !stderr.trim().is_empty() {
            sections.push((t("stderr", "标准错误").to_string(), stderr));
        }
        if sections.is_empty() {
            let fallback = if success {
                t("(no output)", "（无输出）")
            } else {
                t("command failed without output", "命令失败，且没有输出")
            };
            sections.push((t("output", "输出").to_string(), fallback.to_string()));
        }
        return sections;
    }

    parse_stream_sections(body)
}

/// 解析实时缓冲中使用的 `── stdout ──` / `── stderr ──` 分隔符。
///
/// 参数:
/// - `body`: 带分隔符的正文
///
/// 返回:
/// - 解析出的输出流；没有分隔符时返回单个 stdout 流
fn parse_stream_sections(body: &str) -> Vec<(String, String)> {
    let mut sections = Vec::new();
    let mut current_label: Option<String> = None;
    let mut current = String::new();

    for line in body.lines() {
        if let Some(label) = parse_section_label(line) {
            if let Some(previous) = current_label.take() {
                sections.push((previous, std::mem::take(&mut current)));
            }
            current_label = Some(label);
        } else if current_label.is_some() {
            current.push_str(line);
            current.push('\n');
        } else {
            current_label = Some(t("stdout", "标准输出").to_string());
            current.push_str(line);
            current.push('\n');
        }
    }

    if let Some(label) = current_label {
        sections.push((label, current));
    }
    if sections.is_empty() && !body.trim().is_empty() {
        sections.push((t("stdout", "标准输出").to_string(), body.to_string()));
    }
    sections
}

/// 识别命令输出分隔行。
///
/// 参数:
/// - `line`: 待识别行
///
/// 返回:
/// - 标准化标签；非分隔行返回空
fn parse_section_label(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let inner = trimmed.strip_prefix('─')?.strip_suffix('─')?.trim();
    let label = inner.trim_matches('─').trim();
    if label.eq_ignore_ascii_case("stdout") || label == "标准输出" {
        Some(t("stdout", "标准输出").to_string())
    } else if label.eq_ignore_ascii_case("stderr") || label == "标准错误" {
        Some(t("stderr", "标准错误").to_string())
    } else {
        None
    }
}

/// 清理并按内容类型高亮单个输出流。
///
/// 参数:
/// - `content`: 输出流正文
/// - `is_error`: 是否为 stderr
///
/// 返回:
/// - 带代码高亮的正文
fn highlight_output(content: &str, is_error: bool) -> String {
    let content = crate::render::command_result_block::sanitize_command_output(content.trim_end());
    let (content, language) = normalize_json_output(&content);
    let mut rendered = String::new();
    for line in content.lines() {
        let line = highlight_code_line(language, line);
        if is_error {
            rendered.push_str("\x1b[31m");
            rendered.push_str(&line);
            rendered.push_str("\x1b[0m");
        } else {
            rendered.push_str(&line);
        }
        rendered.push('\n');
    }
    if rendered.is_empty() {
        rendered.push_str("\x1b[2m  ");
        rendered.push_str(t("(empty)", "（空）"));
        rendered.push_str("\x1b[0m\n");
    }
    rendered
}

/// 对 JSON 输出去除字符串封装并对对象/数组进行可读格式化。
///
/// 参数:
/// - `content`: 已清理的输出正文
///
/// 返回:
/// - `(可显示文本, 语言标识)`
fn normalize_json_output(content: &str) -> (String, &'static str) {
    let trimmed = content.trim();
    let Ok(value) = serde_json::from_str::<Value>(trimmed) else {
        return (content.to_string(), "");
    };
    match value {
        Value::String(value) => (value, ""),
        Value::Object(_) | Value::Array(_) => (
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| content.to_string()),
            "json",
        ),
        value => (value.to_string(), "json"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::style::{CODE_KEYWORD_STYLE, CODE_STRING_STYLE};

    #[test]
    fn markdown_body_replays_code_highlighting() {
        let output = render_expandable_body(
            ExpandableBlockKind::Markdown,
            "说明\n\n```rust\nfn main() { println!(\"ok\"); }\n```",
        );
        assert!(output.contains("rust"));
        assert!(output.contains(CODE_KEYWORD_STYLE));
        assert!(output.contains(CODE_STRING_STYLE));
        assert!(!output.contains("```"));
    }

    #[test]
    fn markdown_body_preserves_unconfirmed_table_candidate() {
        let output = render_expandable_body(ExpandableBlockKind::Markdown, "| note |");

        assert!(output.contains("| note |"));
    }

    #[test]
    fn command_body_extracts_streams_from_result_json() {
        let body = serde_json::json!({
            "success": true,
            "exit_code": 0,
            "stdout": "{\"name\":\"sai\",\"count\":2}",
            "stderr": ""
        })
        .to_string();
        let output = render_expandable_body(ExpandableBlockKind::Command, &body);
        assert!(output.contains("stdout") || output.contains("标准输出"));
        assert!(output.contains("name"));
        assert!(output.contains("count"));
        assert!(!output.contains("success"));
        assert!(output.contains("json") || output.contains("\x1b["));
    }

    #[test]
    fn command_body_parses_live_stream_sections() {
        let output = render_expandable_body(
            ExpandableBlockKind::Command,
            "── stdout ──\nlet value = 42;\n\n── stderr ──\nfailed",
        );
        assert!(output.contains("let value"));
        assert!(output.contains("failed"));
        assert!(output.contains("stdout") || output.contains("标准输出"));
        assert!(output.contains("stderr") || output.contains("标准错误"));
    }

    #[test]
    fn plain_body_is_not_reinterpreted() {
        let body = "raw text\nsecond line";
        assert_eq!(
            render_expandable_body(ExpandableBlockKind::Plain, body),
            body
        );
    }
}
