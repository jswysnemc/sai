use super::{submit_if_pending, PromptInput, PromptStep, SshPromptView};
use crate::cli::repl_runtime::ReplRuntime;
use crate::cli::terminal_restore;
use crate::ssh::{SecretRequest, SecretResponse};
use anyhow::Result;
use crossterm::event::{self, Event};
use std::io;
use std::time::Duration;

/// 等待按键的轮询间隔；超时后检查后端是否已撤销请求。
const POLL_INTERVAL: Duration = Duration::from_millis(150);

/// 【SSH 征询】【TUI 输入】在输入框上方挂卡片，安全读取秘密或是/否确认。
///
/// 口令输入全程不回显，也不展示长度；应答经独立一次性通道直达后端工具，
/// 不写入 transcript 或模型上下文。
///
/// 参数:
/// - `request`: 待处理的交互征询（不含秘密）
/// - `runtime`: REPL 运行期
///
/// 返回:
/// - 处理结果
pub(in crate::cli) fn prompt_ssh_secret_request_tui(
    request: &SecretRequest,
    runtime: &mut ReplRuntime,
) -> Result<()> {
    let mut stdout = io::stdout();
    // 1. 独占 raw 输入，避免与主循环输入框事件竞争
    let mut terminal_guard = terminal_restore::TerminalInputGuard::enable(&mut stdout, true)?;
    let mut input = PromptInput::new(request);
    let result = read_response(request, runtime, &mut input);
    // 2. 无论成败都先撤卡片，受管区域恢复运行期间的输入框
    let cleared = runtime.clear_ssh_prompt();
    let _ = terminal_guard.finish(&mut stdout);
    submit_if_pending(request, result.unwrap_or(SecretResponse::Cancelled));
    cleared
}

/// 循环读取按键直到得到应答或后端撤销请求。
///
/// 参数:
/// - `request`: 交互征询
/// - `runtime`: REPL 运行期
/// - `input`: 输入状态机
///
/// 返回:
/// - 用户应答；后端撤销时为取消
fn read_response(
    request: &SecretRequest,
    runtime: &mut ReplRuntime,
    input: &mut PromptInput,
) -> Result<SecretResponse> {
    runtime.show_ssh_prompt(SshPromptView::new(request, input))?;
    loop {
        if !event::poll(POLL_INTERVAL)? {
            // 后端超时撤销请求后不再阻塞等待按键
            if !crate::ssh::is_pending(&request.id) {
                return Ok(SecretResponse::Cancelled);
            }
            continue;
        }
        let event = event::read()?;
        if let Event::Resize(cols, rows) = event {
            runtime.observe_stream_resize(cols, rows)?;
            runtime.redraw_stream_composer()?;
            continue;
        }
        match input.handle(event) {
            PromptStep::Continue => {
                runtime.show_ssh_prompt(SshPromptView::new(request, input))?;
            }
            PromptStep::Done(response) => return Ok(response),
        }
    }
}
