//! 终端群组工作台（Super-Agent Group TUI）：支持用户、Leader 与多个 Runner 在统一群组内实时协同。

mod input;
mod state;
mod view;

#[cfg(test)]
mod tests;

use crate::paths::SaiPaths;
use crate::super_agent::channel_bridge::{default_channel_target, ChannelInboundMessage};
use crate::super_agent::{shared_service, SpawnRunnerRequest, SuperAgentService};
use anyhow::Result;
use crossterm::{
    cursor::{Hide, Show},
    execute,
    terminal::{self, Clear, ClearType},
};
use input::GroupAction;
use state::GroupTuiState;
use std::io::{self, Write};
use std::sync::Arc;
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(50);
const REFRESH_INTERVAL: Duration = Duration::from_millis(350);

/// 【群组终端】【屏幕守卫】进入备用屏与原始输入模式，退出或异常时自动恢复终端。
struct GroupScreen {
    _mouse: crate::cli::mouse_capture::MouseCapture,
    was_raw: bool,
    entered: bool,
    stdout: io::Stdout,
    previous: Vec<String>,
    size: Option<(u16, u16)>,
}

impl GroupScreen {
    /// 启动群组 TUI 备用屏幕。
    fn start() -> Result<Self> {
        let mut screen = Self {
            _mouse: crate::cli::mouse_capture::MouseCapture::acquire()?,
            was_raw: terminal::is_raw_mode_enabled()?,
            entered: false,
            stdout: io::stdout(),
            previous: Vec::new(),
            size: None,
        };
        terminal::enable_raw_mode()?;
        crate::cli::alternate_screen::enter_alternate_screen(&mut screen.stdout)?;
        screen.entered = true;
        execute!(screen.stdout, Hide)?;
        write!(
            screen.stdout,
            "{}",
            crate::render::terminal_image::kitty_delete_placements()
        )?;
        Ok(screen)
    }

    /// 增量绘制发生变化的屏幕行。
    fn draw(&mut self, lines: &[String], size: (u16, u16)) -> Result<()> {
        if self.size == Some(size) && self.previous == lines {
            return Ok(());
        }
        crate::config_tui::ui::begin_synced_frame(&mut self.stdout)?;
        if self.size != Some(size) {
            execute!(self.stdout, Clear(ClearType::All))?;
            self.previous.clear();
        }
        let readable: Vec<_> = lines
            .iter()
            .map(|line| crate::render::readable_terminal_text(line))
            .collect();
        crate::render::terminal_rows::paint_changed_rows(
            &mut self.stdout,
            0,
            usize::from(size.0),
            &readable,
            Some(&self.previous),
        )?;
        crate::config_tui::ui::end_synced_frame(&mut self.stdout)?;
        self.previous = lines.to_vec();
        self.size = Some(size);
        Ok(())
    }
}

impl Drop for GroupScreen {
    fn drop(&mut self) {
        let _ = execute!(self.stdout, Show);
        if self.entered {
            let _ = crate::cli::alternate_screen::leave_alternate_screen(&mut self.stdout);
        }
        if !self.was_raw {
            let _ = terminal::disable_raw_mode();
        }
    }
}

/// 【群组终端】【主入口】启动交互式 Super-Agent 群组工作台，支持与 Leader 及 Runner 实时对话与编排。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `initial_group`: 可选初始打开的群组 ID 或名称
///
/// 返回:
/// - 面板退出结果
pub(in crate::cli) async fn run_group_tui(
    paths: &SaiPaths,
    initial_group: Option<&str>,
) -> Result<()> {
    let service = shared_service(paths)?;
    service.ensure_watchdog_started();
    let _ = service.ensure_default_group(None)?;

    let mut state = GroupTuiState::new();
    state.reload(&service, initial_group)?;
    let mut events_rx = service.subscribe_events();
    let mut screen = GroupScreen::start()?;
    let mut last_refresh = Instant::now();

    loop {
        let size = terminal::size().unwrap_or((120, 32));
        let frame = view::render(&mut state, size);
        screen.draw(&frame, size)?;

        // 1. 检查是否收到来自 Leader / Runner / 渠道的实时事件
        let mut dirty = false;
        while events_rx.try_recv().is_ok() {
            dirty = true;
        }
        if dirty || last_refresh.elapsed() >= REFRESH_INTERVAL {
            let _ = state.reload(&service, None);
            last_refresh = Instant::now();
        }

        // 2. 轮询键盘与鼠标事件
        if !crate::cli::terminal_events::poll(POLL_INTERVAL)? {
            continue;
        }
        let ev = crate::cli::terminal_events::read()?;
        let action = input::handle_event(&mut state, ev);
        match execute_action(&service, &mut state, action).await {
            Ok(true) => break,
            Ok(false) => {}
            Err(error) => {
                state.set_notice(format!("操作失败：{error:#}"));
            }
        }
    }
    Ok(())
}

/// 执行界面交互产生的业务动作。
async fn execute_action(
    service: &Arc<SuperAgentService>,
    state: &mut GroupTuiState,
    action: GroupAction,
) -> Result<bool> {
    match action {
        GroupAction::None => Ok(false),
        GroupAction::Exit => Ok(true),
        GroupAction::Reload => {
            state.reload(service, None)?;
            Ok(false)
        }
        GroupAction::SendChat {
            group_id,
            content,
            recipient_id,
        } => {
            let (_, decision) = service
                .post_user_message(
                    &group_id,
                    "You",
                    &content,
                    None,
                    recipient_id.as_deref(),
                    "tui",
                    true,
                )
                .await?;
            state.chat_scroll = 0;
            state.set_notice(format!("已发送（{}）", decision.route_path()));
            state.reload(service, Some(&group_id))?;
            Ok(false)
        }
        GroupAction::CreateGroup { name, description } => {
            let (group, _) = service.create_group(&name, &description, "", None)?;
            state.set_notice(format!("已创建群组「{}」", group.name));
            state.reload(service, Some(&group.id))?;
            Ok(false)
        }
        GroupAction::SpawnRunner {
            group_id,
            name,
            specialty,
            prompt,
        } => {
            let (member, task) = service
                .spawn_runner(SpawnRunnerRequest {
                    group_id: group_id.clone(),
                    name,
                    specialty,
                    subagent_type: "general".to_string(),
                    task_title: prompt.chars().take(42).collect(),
                    prompt,
                    thread_id: None,
                    origin_channel: Some("tui".to_string()),
                    origin_target: None,
                })
                .await?;
            state.chat_scroll = 0;
            state.set_notice(format!(
                "已启动 Runner「{}」执行任务 {}",
                member.name, task.id
            ));
            state.reload(service, Some(&group_id))?;
            Ok(false)
        }
        GroupAction::StopSelectedRunner {
            group_id,
            member_id,
            member_name,
        } => {
            service.stop_runner(&group_id, &member_id, "stopped from TUI")?;
            state.set_notice(format!("已停止 Runner「{member_name}」"));
            state.reload(service, Some(&group_id))?;
            Ok(false)
        }
        GroupAction::ChannelInbound {
            group_id,
            channel,
            chat_id,
            actor,
            content,
        } => {
            let (gid, _, decision) = service
                .ingest_channel_message(
                    ChannelInboundMessage {
                        channel: channel.clone(),
                        external_chat_id: chat_id.clone(),
                        actor,
                        content,
                        group_id: Some(group_id),
                        reply_target: default_channel_target(&chat_id, true),
                        simulate_outbound: true,
                    },
                    true,
                )
                .await?;
            state.chat_scroll = 0;
            state.set_notice(format!(
                "已注入 {channel} 渠道消息（{}）",
                decision.route_path()
            ));
            state.reload(service, Some(&gid))?;
            Ok(false)
        }
        GroupAction::BindChannel {
            group_id,
            channel,
            chat_id,
        } => {
            service.bind_channel(
                &group_id,
                &channel,
                &chat_id,
                &format!("{channel}:{chat_id}"),
                "auto",
                true,
            )?;
            state.set_notice(format!("已绑定渠道 {channel}:{chat_id}"));
            state.reload(service, Some(&group_id))?;
            Ok(false)
        }
        GroupAction::RunWatchdog => {
            let events = service.run_watchdog_check().await?;
            state.set_notice(format!("Watchdog 巡检完成，产出 {} 条记录", events.len()));
            state.reload(service, None)?;
            Ok(false)
        }
    }
}
