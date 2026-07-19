use super::ReplRuntime;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use std::time::Duration;

impl ReplRuntime {
    /// 保存模型运行期间收到的普通终端输入。
    ///
    /// 参数:
    /// - `event`: 待交给下一次输入框处理的事件
    ///
    /// 返回:
    /// - 无
    pub(in crate::cli) fn queue_input_event(&mut self, event: Event) {
        self.pending_input_events.push_back(event);
    }

    /// 读取模型运行期间保存的最早终端输入。
    ///
    /// 返回:
    /// - 下一条待处理事件
    pub(in crate::cli) fn pop_input_event(&mut self) -> Option<Event> {
        self.pending_input_events.pop_front()
    }

    /// 切换最近命令输出的展开状态并重绘 TUI。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 是否找到可切换的命令输出
    pub(in crate::cli) fn toggle_command_output(&mut self) -> Result<bool> {
        if !self.transcript.toggle_latest_command_output() {
            return Ok(false);
        }
        self.replay(false)?;
        Ok(true)
    }
}

/// 在流式事件循环 tick 中采样尺寸并执行到期 reflow 与 live 刷新。
///
/// 参数:
/// - `runtime`: 当前 REPL 运行期
///
/// 返回:
/// - 处理是否成功
pub(crate) fn process_stream_tick(runtime: &mut ReplRuntime) -> Result<()> {
    runtime.observe_terminal_size(true)?;
    runtime.maybe_reflow_due(true)?;
    runtime.tick_live()?;
    runtime.tick_subagents().map(|_| ())
}

/// 处理模型运行期间的非阻塞终端事件。
///
/// 参数:
/// - `runtime`: 当前 REPL 运行期
///
/// 返回:
/// - 收到 Ctrl+C 时返回 true
pub(crate) fn process_stream_input(runtime: &mut ReplRuntime) -> Result<bool> {
    while event::poll(Duration::ZERO)? {
        let input = event::read()?;
        match input {
            Event::Resize(cols, rows) => runtime.observe_input_resize(cols, rows),
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                if matches!(key.code, KeyCode::Char('o'))
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    runtime.toggle_command_output()?;
                } else if matches!(key.code, KeyCode::Char('c'))
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    return Ok(true);
                } else {
                    runtime.queue_input_event(Event::Key(key));
                }
            }
            Event::Key(_) => {}
            input => runtime.queue_input_event(input),
        }
    }
    Ok(false)
}
