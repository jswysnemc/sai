//! 终端群组工作台（Super-Agent Group TUI）：以 Sai 原生全屏风格承载用户、Leader 与多个 Runner 的实时群组协作，
//! 复用原生 `repl_chrome` 输入区、实时展示模型基础信息与上下文占用，并支持外部渠道接入与工具边界检视。

mod chat_view;
mod completion;
mod composer;
mod header_view;
mod input;
mod inspector_view;
mod modal_input;
mod modal_state;
mod modal_view;
mod model_info;
mod sidebar_view;
mod state;
mod text_util;
mod view;

#[cfg(test)]
mod tests;

use crate::cli::keyboard_enhancement::KeyboardEnhancementState;
use crate::cli::models_picker::{run_interactive as run_model_picker, PickerOutcome};
use crate::config::AppConfig;
use crate::gateways::manager::{start_gateway, stop_gateway, ManagedGateway};
use crate::paths::SaiPaths;
use crate::super_agent::channel_bridge::{default_channel_target, ChannelInboundMessage};
use crate::super_agent::model::SuperAgentEvent;
use crate::super_agent::{shared_service, SpawnRunnerRequest, SuperAgentService};
use anyhow::Result;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{DisableBracketedPaste, EnableBracketedPaste},
    execute, queue,
    terminal::{self, Clear, ClearType},
};
use input::GroupAction;
use state::{GroupTuiState, InspectorTab};
use std::io::{self, Write};
use std::sync::Arc;
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(50);
const REFRESH_INTERVAL: Duration = Duration::from_millis(350);
const SPINNER_INTERVAL: Duration = Duration::from_millis(120);

/// 【群组终端】【屏幕守卫】管理备用屏、原始模式、括号粘贴、键盘增强与真实硬件光标。
struct GroupScreen {
    mouse: Option<crate::cli::mouse_capture::MouseCapture>,
    enhancement: KeyboardEnhancementState,
    was_raw: bool,
    entered: bool,
    stdout: io::Stdout,
    previous: Vec<String>,
    last_cursor: Option<(u16, u16)>,
    size: Option<(u16, u16)>,
}

impl GroupScreen {
    /// 启动群组 TUI 备用屏幕及完整输入模式。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 已初始化的屏幕守卫
    fn start() -> Result<Self> {
        let mut screen = Self {
            mouse: Some(crate::cli::mouse_capture::MouseCapture::acquire()?),
            enhancement: KeyboardEnhancementState::default(),
            was_raw: terminal::is_raw_mode_enabled()?,
            entered: false,
            stdout: io::stdout(),
            previous: Vec::new(),
            last_cursor: None,
            size: None,
        };
        terminal::enable_raw_mode()?;
        crate::cli::alternate_screen::enter_alternate_screen(&mut screen.stdout)?;
        screen.entered = true;
        execute!(screen.stdout, EnableBracketedPaste, Hide)?;
        screen.enhancement = KeyboardEnhancementState::enable(&mut screen.stdout);
        write!(
            screen.stdout,
            "{}",
            crate::render::terminal_image::kitty_delete_placements()
        )?;
        Ok(screen)
    }

    /// 增量绘制发生变化的屏幕行，并同步定位真实硬件光标以支持原生中文输入法。
    ///
    /// 参数:
    /// - `frame`: 整屏行缓冲与期望光标坐标
    /// - `size`: 当前终端 `(cols, rows)`
    ///
    /// 返回:
    /// - 绘制结果
    fn draw(&mut self, frame: &view::RenderedFrame, size: (u16, u16)) -> Result<()> {
        let readable: Vec<String> = frame
            .lines
            .iter()
            .map(|line| crate::render::readable_terminal_text(line))
            .collect();
        let rows_changed = self.size != Some(size) || self.previous != readable;
        let cursor_changed = self.last_cursor != frame.cursor;
        if !rows_changed && !cursor_changed {
            return Ok(());
        }

        crate::config_tui::ui::begin_synced_frame(&mut self.stdout)?;
        if rows_changed {
            queue!(self.stdout, Hide)?;
            if self.size != Some(size) {
                queue!(self.stdout, Clear(ClearType::All))?;
                self.previous.clear();
            }
            crate::render::terminal_rows::paint_changed_rows(
                &mut self.stdout,
                0,
                usize::from(size.0),
                &readable,
                Some(&self.previous),
            )?;
            self.previous = readable;
            self.size = Some(size);
        }
        match frame.cursor {
            Some((col, row)) => {
                queue!(self.stdout, MoveTo(col, row), Show)?;
            }
            None => {
                queue!(self.stdout, Hide)?;
            }
        }
        crate::config_tui::ui::end_synced_frame(&mut self.stdout)?;
        self.last_cursor = frame.cursor;
        Ok(())
    }

    /// 清除屏幕缓存以在下帧触发全屏重绘（供 Ctrl+L 刷新使用）。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    fn invalidate(&mut self) {
        self.previous.clear();
        self.size = None;
        self.last_cursor = None;
    }

    /// 临时挂起群组备用屏以运行独立交互子控件（如 `/model` 模型选择器），并在回调结束后完整恢复。
    ///
    /// 参数:
    /// - `f`: 子交互闭包
    ///
    /// 返回:
    /// - 子交互结果
    fn suspend_for<T>(&mut self, f: impl FnOnce() -> Result<T>) -> Result<T> {
        self.enhancement.disable(&mut self.stdout);
        let _ = execute!(self.stdout, DisableBracketedPaste, Show);
        self.mouse.take();
        if self.entered {
            let _ = crate::cli::alternate_screen::leave_alternate_screen(&mut self.stdout);
            self.entered = false;
        }
        if !self.was_raw {
            let _ = terminal::disable_raw_mode();
        }
        let outcome = f();
        // 无论子交互成功与否都恢复群组备用屏并强制整屏重绘
        let _ = terminal::enable_raw_mode();
        if crate::cli::alternate_screen::enter_alternate_screen(&mut self.stdout).is_ok() {
            self.entered = true;
        }
        self.mouse = crate::cli::mouse_capture::MouseCapture::acquire().ok();
        let _ = execute!(self.stdout, EnableBracketedPaste, Hide);
        self.enhancement = KeyboardEnhancementState::enable(&mut self.stdout);
        self.previous.clear();
        self.size = None;
        self.last_cursor = None;
        outcome
    }
}

impl Drop for GroupScreen {
    fn drop(&mut self) {
        self.enhancement.disable(&mut self.stdout);
        let _ = execute!(self.stdout, DisableBracketedPaste, Show);
        self.mouse.take();
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
    // 1. 进入原始模式前完成终端明暗底色探测，保证独立启动 `sai group` 与主 REPL 共享一致配色
    crate::render::terminal_palette::initialize_terminal_palette();
    let service = shared_service(paths)?;
    service.ensure_watchdog_started();
    let _ = service.ensure_default_group(None)?;

    let mut state = GroupTuiState::new();
    state.reload(&service, initial_group)?;
    let mut events_rx = service.subscribe_events();
    let mut screen = GroupScreen::start()?;
    let mut last_refresh = Instant::now();
    let mut last_spin = Instant::now();

    loop {
        state.expire_notice();
        if last_spin.elapsed() >= SPINNER_INTERVAL {
            state.tick = state.tick.wrapping_add(1);
            last_spin = Instant::now();
        }
        let size = terminal::size().unwrap_or((120, 32));
        let frame = view::render(&mut state, size);
        screen.draw(&frame, size)?;

        // 1. 消费来自 Leader / Runner / 渠道的实时增量事件（含思考、工具调用与流式草稿）
        let mut dirty = false;
        while let Ok(ev) = events_rx.try_recv() {
            if apply_live_event(&mut state, &ev) {
                dirty = true;
            }
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
        match execute_action(paths, &service, &mut state, &mut screen, action).await {
            Ok(true) => break,
            Ok(false) => {}
            Err(error) => {
                state.set_error(format!("操作失败：{error:#}"));
            }
        }
    }
    Ok(())
}

/// 将单条实时事件合并进视图态，返回是否需要重新加载数据库快照。
fn apply_live_event(state: &mut GroupTuiState, event: &SuperAgentEvent) -> bool {
    let active_gid = state.selected_group_id();
    let matches_group = match (active_gid, event.group_id.as_deref()) {
        (Some(active), Some(ev_gid)) => active == ev_gid,
        (_, None) => true,
        (None, Some(_)) => true,
    };
    if !matches_group {
        return false;
    }
    match event.kind.as_str() {
        "leader.thinking" => {
            state.leader_live.thinking = true;
            state.leader_live.activity = "正在研判并编排任务...".to_string();
            state.leader_live.draft_preview.clear();
            false
        }
        "leader.activity" => {
            state.leader_live.thinking = true;
            if let Some(act) = event.payload.get("activity").and_then(|v| v.as_str()) {
                state.leader_live.activity = act.to_string();
            }
            false
        }
        "leader.delta" => {
            state.leader_live.thinking = true;
            if let Some(preview) = event.payload.get("preview").and_then(|v| v.as_str()) {
                state.leader_live.draft_preview = preview.to_string();
            }
            false
        }
        "leader.idle" => {
            state.leader_live = Default::default();
            true
        }
        _ => true,
    }
}

/// 执行界面交互产生的业务动作。
async fn execute_action(
    paths: &SaiPaths,
    service: &Arc<SuperAgentService>,
    state: &mut GroupTuiState,
    screen: &mut GroupScreen,
    action: GroupAction,
) -> Result<bool> {
    match action {
        GroupAction::None => Ok(false),
        GroupAction::Exit => Ok(true),
        GroupAction::Redraw => {
            screen.invalidate();
            Ok(false)
        }
        GroupAction::Reload => {
            state.reload(service, None)?;
            Ok(false)
        }
        GroupAction::SelectModel => {
            match screen.suspend_for(|| run_model_picker(paths)) {
                Ok(PickerOutcome::Saved { message, .. }) => {
                    state.set_notice(format!("已切换模型：{message}"));
                }
                Ok(PickerOutcome::Cancelled) => {
                    state.set_notice("已取消模型切换");
                }
                Err(error) => {
                    state.set_error(format!("模型切换失败：{error:#}"));
                }
            }
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
            state.unseen_chat = false;
            let route_label = match decision.route_path() {
                "fast_path" => "直通 Runner",
                "slow_path" => "已送达 Leader",
                _ => "已过滤",
            };
            state.set_notice(format!("已发送（{route_label}）"));
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
            state.unseen_chat = false;
            state.set_notice(format!("已启动 Runner「{}」（{}）", member.name, task.id));
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
            state.unseen_chat = false;
            state.set_notice(format!(
                "已模拟注入 {channel} 消息（{}）",
                decision.route_path()
            ));
            state.reload(service, Some(&gid))?;
            Ok(false)
        }
        GroupAction::BindChannel {
            group_id,
            channel,
            chat_id,
            admission,
        } => {
            service.bind_channel(
                &group_id,
                &channel,
                &chat_id,
                &format!("{channel}:{chat_id}"),
                &admission,
                true,
            )?;
            state.inspector_open = true;
            state.inspector_tab = InspectorTab::ChannelsGateways;
            state.set_notice(format!("已绑定渠道 {channel}:{chat_id}（{admission}）"));
            state.reload(service, Some(&group_id))?;
            Ok(false)
        }
        GroupAction::GatewayControl { action, target } => {
            handle_gateway_control(paths, state, &action, &target).await?;
            state.reload(service, None)?;
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

/// 在群组 TUI 内直接查看或启停微信 / QQ 后台网关进程。
async fn handle_gateway_control(
    paths: &SaiPaths,
    state: &mut GroupTuiState,
    action: &str,
    target: &str,
) -> Result<()> {
    let config = AppConfig::load_or_default(paths)?;
    let gw = match target {
        "weixin" | "wechat" | "wx" => Some(ManagedGateway::Weixin),
        "qq" => Some(ManagedGateway::Qq),
        _ => None,
    };
    state.inspector_open = true;
    state.inspector_tab = InspectorTab::ChannelsGateways;
    match action {
        "start" => {
            let Some(managed) = gw else {
                state.set_error("支持启停的后台网关：weixin、qq（飞书请运行 sai gateway start）");
                return Ok(());
            };
            if managed == ManagedGateway::Weixin && !state.gateway_status.weixin_logged_in {
                state.set_error("微信尚未登录：请先在终端运行 sai gateway weixin-login 完成扫码");
                return Ok(());
            }
            let _ = start_gateway(paths, &config, managed).await?;
            state.set_notice(format!("已启动 {} 后台网关", managed.title()));
        }
        "stop" => {
            let Some(managed) = gw else {
                state.set_error("用法：/gateway stop <weixin|qq>");
                return Ok(());
            };
            let stopped = stop_gateway(paths, &config, managed).await?;
            state.set_notice(format!(
                "已停止 {} 后台网关（{} 个进程）",
                managed.title(),
                stopped
            ));
        }
        _ => {
            let status = state::GatewayStatusSummary::probe(paths);
            let wx = if status.weixin_running {
                "微信:运行中"
            } else if status.weixin_logged_in {
                "微信:已登录未启动"
            } else {
                "微信:未登录"
            };
            let qq = if status.qq_running {
                "QQ:运行中"
            } else {
                "QQ:未启动"
            };
            state.set_notice(format!("{wx} · {qq}（可用 /gateway start weixin）"));
        }
    }
    Ok(())
}
