use super::{submit_if_pending, PromptInput, PromptStep, SshPromptView};
use crate::ssh::{SecretRequest, SecretResponse};
use anyhow::Result;
use crossterm::event::{self, DisableBracketedPaste, EnableBracketedPaste};
use crossterm::execute;
use std::io::{self, IsTerminal, Write};
use std::time::Duration;

/// 等待按键的轮询间隔；超时后检查后端是否已撤销请求。
const POLL_INTERVAL: Duration = Duration::from_millis(150);

/// 【SSH 征询】【CLI 输入】在流式输出中逐行打印卡片并安全读取应答。
///
/// 非交互终端无法安全输入，直接取消并交由后端给出明确提示。
///
/// 参数:
/// - `request`: 待处理的交互征询（不含秘密）
///
/// 返回:
/// - 处理结果
pub(in crate::cli) fn prompt_ssh_secret_request_cli(request: &SecretRequest) -> Result<()> {
    let mut stdout = io::stdout();
    if !(io::stdin().is_terminal() && stdout.is_terminal()) {
        submit_if_pending(request, SecretResponse::Cancelled);
        return Ok(());
    }
    let mut input = PromptInput::new(request);
    let cols = crossterm::terminal::size()
        .map(|(cols, _)| usize::from(cols))
        .unwrap_or(80);
    // 1. 卡片与提示逐行输出，末行留给输入
    let view = SshPromptView::new(request, &input);
    writeln!(stdout)?;
    for line in view.panel_lines(cols) {
        writeln!(stdout, "{line}")?;
    }
    write!(stdout, "  \x1b[2m{}\x1b[0m\n› ", view.placeholder())?;
    stdout.flush()?;
    // 2. raw 模式读取，按键不回显
    crossterm::terminal::enable_raw_mode()?;
    let _ = execute!(stdout, EnableBracketedPaste);
    let response = read_response(request, &mut input).unwrap_or(SecretResponse::Cancelled);
    let _ = execute!(stdout, DisableBracketedPaste);
    let _ = crossterm::terminal::disable_raw_mode();
    println!();
    submit_if_pending(request, response);
    Ok(())
}

/// 循环读取按键直到得到应答或后端撤销请求。
///
/// 参数:
/// - `request`: 交互征询
/// - `input`: 输入状态机
///
/// 返回:
/// - 用户应答
fn read_response(request: &SecretRequest, input: &mut PromptInput) -> Result<SecretResponse> {
    loop {
        if !event::poll(POLL_INTERVAL)? {
            if !crate::ssh::is_pending(&request.id) {
                return Ok(SecretResponse::Cancelled);
            }
            continue;
        }
        if let PromptStep::Done(response) = input.handle(event::read()?) {
            return Ok(response);
        }
    }
}
