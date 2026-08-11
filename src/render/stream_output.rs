use crate::llm::ChatResult;
use crate::render::markdown::MarkdownStreamRenderer;
use crate::render::terminal_text as t;
use crate::render::transcript::reasoning_cell;
use anyhow::Result;
use crossterm::style::{Color, ResetColor, SetForegroundColor};
use crossterm::{execute, terminal};
use std::io::{self, Write};

/// 打印一次完整助手回复。
///
/// 参数:
/// - `response`: 聊天结果
/// - `show_reasoning`: 是否打印推理内容
///
/// 返回:
/// - 打印是否成功
pub fn print_assistant_response(response: &ChatResult, show_reasoning: bool) -> Result<()> {
    if show_reasoning {
        if let Some(reasoning) = response
            .reasoning
            .as_deref()
            .filter(|text| !text.trim().is_empty())
        {
            print_reasoning(reasoning)?;
        }
    }
    print_markdown(&response.content);
    Ok(())
}

/// 打印 Markdown 文本。
///
/// 参数:
/// - `markdown`: 原始 Markdown 文本
///
/// 返回:
/// - 无
pub fn print_markdown(markdown: &str) {
    let mut renderer = MarkdownStreamRenderer::new_stable();
    let markdown = markdown.trim_end();
    let rendered = crate::render::render_width::with_render_width(
        crate::render::content_indent::cli_content_width(),
        || {
            let mut rendered = renderer.push(markdown);
            rendered.push_str(&renderer.flush());
            rendered
        },
    );
    let rendered = crate::render::content_indent::wrap_cli_stream_block(&rendered);
    let rendered = crate::render::content_indent::align_cli_stream_block(&rendered);
    let mut stdout = io::stdout();
    let _ = write!(stdout, "{rendered}");
    let _ = stdout.flush();
}

/// 打印推理内容块。
///
/// 参数:
/// - `reasoning`: 推理内容
///
/// 返回:
/// - 打印是否成功
fn print_reasoning(reasoning: &str) -> Result<()> {
    let mut stdout = io::stdout();
    execute!(stdout, SetForegroundColor(Color::DarkCyan))?;
    // 非流式打印也用 ◦，与 TUI / CLI 流式思考标题一致
    writeln!(
        stdout,
        "{} {}",
        reasoning_cell::THINKING_MARKER,
        t("thinking", "思考")
    )?;
    for line in reasoning.trim().lines() {
        writeln!(stdout, "  {line}")?;
    }
    execute!(stdout, ResetColor)?;
    if terminal::size().is_ok() {
        writeln!(stdout)?;
    }
    Ok(())
}
