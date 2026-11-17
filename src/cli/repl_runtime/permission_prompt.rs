use super::ReplRuntime;
use crate::agent::{AgentMode, PermissionMode};
use crate::permission::{
    self, PermissionDecision, PermissionInteractionState, PermissionRequest, PermissionTransition,
};
use crate::render::PermissionChoice;
use anyhow::Result;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use std::time::{Duration, Instant};

/// 流式期间权限弹窗的防误触保护时长。
const PERMISSION_CONFIRM_GUARD: Duration = Duration::from_millis(300);

pub(super) struct PendingPermissionPrompt {
    request: PermissionRequest,
    state: PermissionInteractionState,
    #[cfg_attr(test, allow(dead_code))]
    opened_at: Instant,
    #[cfg_attr(test, allow(dead_code))]
    guard_confirm: bool,
}

/// 【权限】【防误触判定】流式打字期间刚弹出审批卡片时拦截直接确认键，保留方向键与拒绝键。
/// 参数: `key` 为键盘事件，`elapsed` 为弹窗已展示时长，`guard_confirm` 为是否启用保护，`reply_editing` 为是否在编辑拒绝理由；返回: 是否应忽略本次按键。
fn should_guard_confirm_key(
    key: &KeyEvent,
    elapsed: Duration,
    guard_confirm: bool,
    reply_editing: bool,
) -> bool {
    guard_confirm
        && !reply_editing
        && elapsed < PERMISSION_CONFIRM_GUARD
        && key.kind != KeyEventKind::Release
        && matches!(key.code, KeyCode::Enter | KeyCode::Char('y' | 'Y' | '1'))
}

/// 【权限】【选项识别】从终端行纯文本与点击列解析命中的审批选项，行尾空白不触发。
/// 参数: `line` 为 ANSI 渲染行，`column` 为终端列坐标；返回: 命中的权限选项。
fn parse_permission_option_line(line: &str, column: u16) -> Option<PermissionChoice> {
    let plain = crate::render::ansi_clip::strip_ansi(line);
    if usize::from(column) >= crate::cli::repl_text::visible_width(&plain) {
        return None;
    }
    let cleaned = plain
        .trim_start()
        .strip_prefix('│')
        .unwrap_or(plain.trim_start())
        .trim_start();
    let cleaned = cleaned.strip_prefix('❯').unwrap_or(cleaned).trim();
    PermissionChoice::all()
        .into_iter()
        .find(|choice| choice.label() == cleaned)
}

impl ReplRuntime {
    /// 【权限】【空闲命令】参数 request 为插件审批；异步等待输入并保留运行时调度，返回交互结果。
    pub(in crate::cli) async fn wait_for_permission_prompt(
        &mut self,
        request: PermissionRequest,
    ) -> Result<()> {
        let mut stdout = std::io::stdout();
        let mut guard =
            crate::cli::terminal_restore::TerminalInputGuard::enable(&mut stdout, true)?;
        let result = async {
            self.begin_permission_prompt(request.clone())?;
            while permission::is_permission_pending(&request.id) {
                if crossterm::event::poll(Duration::ZERO)? {
                    self.handle_permission_input(&crossterm::event::read()?)?;
                } else {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            }
            self.refresh_permission_prompt()
        }
        .await;
        let restored = guard.finish(&mut stdout);
        result.and(restored)
    }

    /// 【权限】【交互开始】参数 request 为待审操作；登记卡片并返回主循环，返回绘制结果。
    pub(in crate::cli) fn begin_permission_prompt(
        &mut self,
        request: PermissionRequest,
    ) -> Result<()> {
        // 1. 【权限】【草稿防误触】仅在用户已在草稿区输入排队文本时拦截前 300ms 直接确认键，避免吞掉空闲态立即确认
        let guard_confirm = !self.stream_draft.text.is_empty();
        self.pause_for_permission_prompt()?;
        self.record_permission_request(request.clone())?;
        self.permission_prompts.push_back(PendingPermissionPrompt {
            request,
            state: PermissionInteractionState::new(),
            opened_at: Instant::now(),
            guard_confirm,
        });
        self.paint_permission_prompt()
    }

    /// 【权限】【过期清理】无参数；移除已超时、取消或自动审核完成的交互，返回绘制结果。
    pub(super) fn refresh_permission_prompt(&mut self) -> Result<()> {
        let count = self.permission_prompts.len();
        self.permission_prompts
            .retain(|prompt| permission::is_permission_pending(&prompt.request.id));
        if count != self.permission_prompts.len() {
            self.paint_permission_prompt()?;
        }
        Ok(())
    }

    /// 【权限】【坐标定位】定位全屏或内联画布上指定屏幕坐标处的权限选项行。
    /// 参数: `column` 为屏幕列，`row` 为屏幕行；返回: 命中的权限选项。
    fn permission_choice_at(&self, column: u16, row: u16) -> Option<PermissionChoice> {
        // 1. 【权限】【全屏定位】全屏模式下从当前可见正文行映射文档行
        if self.fullscreen.is_some() {
            let line = self.fullscreen_body_line_at(column, row)?;
            return parse_permission_option_line(line, column);
        }
        // 2. 【权限】【内联定位】内联模式下按受管历史窗口起点换算全局行
        let viewport = self.drawing_viewport();
        if row < viewport.origin_row()
            || row >= viewport.composer_top()
            || column >= viewport.size().cols
        {
            return None;
        }
        let global = self.stream.offscreen() + usize::from(row - viewport.origin_row());
        let line = self.stream.line_at(global)?.as_str();
        parse_permission_option_line(line, column)
    }

    /// 【权限】【事件处理】参数 input 为已就绪事件；返回是否由审批消费，不进行阻塞读取。
    pub(super) fn handle_permission_input(&mut self, input: &Event) -> Result<bool> {
        self.refresh_permission_prompt()?;
        if self.permission_prompts.is_empty() {
            return Ok(false);
        }
        let clicked_choice = match input {
            Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left) => {
                self.permission_choice_at(mouse.column, mouse.row)
            }
            _ => None,
        };
        let fullscreen_active = self.fullscreen.is_some();
        let Some(prompt) = self.permission_prompts.front_mut() else {
            return Ok(false);
        };
        let request_id = prompt.request.id.clone();
        // 1. 【权限】【中断与模式】拒绝当前操作或保留工具方式切换 YOLO
        if crate::cli::permission_prompt::is_interrupt(input) {
            permission::decide_permission(&request_id, PermissionDecision::Deny { reply: None })?;
            self.refresh_permission_prompt()?;
            return Ok(true);
        }
        if let Event::Key(key) = input {
            #[cfg(not(test))]
            if should_guard_confirm_key(
                key,
                prompt.opened_at.elapsed(),
                prompt.guard_confirm,
                prompt.state.reply_draft().is_some(),
            ) {
                return Ok(true);
            }
            let shift_tab = key.kind != KeyEventKind::Release
                && (key.code == KeyCode::BackTab
                    || (key.code == KeyCode::Tab && key.modifiers.contains(KeyModifiers::SHIFT)));
            if shift_tab {
                let mode = self
                    .stream_mode(AgentMode::Yolo)
                    .with_permission(PermissionMode::Yolo);
                self.stream_draft_mut().mode = Some(mode);
                self.apply_stream_mode_live(mode);
                self.refresh_permission_prompt()?;
                self.record_meta(format!(
                    "{}: {}",
                    crate::i18n::text("mode", "模式"),
                    mode.label()
                ))?;
                return Ok(true);
            }
        }
        // 2. 【权限】【输入分派】支持左键直接点选选项、全屏滚轮浏览与内联滚轮切项
        if let Event::Resize(cols, rows) = input {
            self.observe_stream_resize(*cols, *rows)?;
        } else if let Event::Mouse(mouse) = input {
            if let Some(choice) = clicked_choice.filter(|_| prompt.state.reply_draft().is_none()) {
                let digit = match choice {
                    PermissionChoice::Allow => '1',
                    PermissionChoice::Deny => '2',
                    PermissionChoice::DenyWithReply => '3',
                };
                let synthetic =
                    Event::Key(KeyEvent::new(KeyCode::Char(digit), KeyModifiers::NONE));
                if let PermissionTransition::Submit(decision) = prompt.state.handle_event(synthetic)
                {
                    if permission::is_permission_pending(&request_id) {
                        permission::decide_permission(&request_id, decision)?;
                    }
                    self.refresh_permission_prompt()?;
                }
            } else if fullscreen_active {
                self.handle_fullscreen_event(input)?;
            } else if prompt.state.reply_draft().is_none()
                && matches!(
                    mouse.kind,
                    MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
                )
            {
                let nav = if mouse.kind == MouseEventKind::ScrollUp {
                    KeyCode::Up
                } else {
                    KeyCode::Down
                };
                let _ = prompt
                    .state
                    .handle_event(Event::Key(KeyEvent::new(nav, KeyModifiers::NONE)));
            }
        } else if let PermissionTransition::Submit(decision) =
            prompt.state.handle_event(input.clone())
        {
            if permission::is_permission_pending(&request_id) {
                permission::decide_permission(&request_id, decision)?;
            }
            self.refresh_permission_prompt()?;
        }
        self.paint_permission_prompt()?;
        Ok(true)
    }

    /// 【权限】【选项绘制】无参数；同步队首审批选项与拒绝理由，返回绘制结果。
    fn paint_permission_prompt(&mut self) -> Result<()> {
        let Some(prompt) = self.permission_prompts.front() else {
            return Ok(());
        };
        let request_id = prompt.request.id.clone();
        let selected = prompt.state.selected();
        let draft = prompt.state.reply_draft().map(str::to_string);
        self.update_permission_choice(&request_id, selected)?;
        self.update_permission_reply(&request_id, draft)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【权限测试】【防误触窗口】验证首 300ms 仅拦截直接确认键，放行方向键与拒绝键。
    /// 参数: 无；返回: 无。
    #[test]
    fn confirm_guard_blocks_only_blind_approval_keys_within_window() {
        let within = Duration::from_millis(120);
        let after = Duration::from_millis(320);
        for code in [
            KeyCode::Enter,
            KeyCode::Char('y'),
            KeyCode::Char('Y'),
            KeyCode::Char('1'),
        ] {
            let event = KeyEvent::new(code, KeyModifiers::NONE);
            assert!(should_guard_confirm_key(&event, within, true, false));
            assert!(!should_guard_confirm_key(&event, after, true, false));
            assert!(!should_guard_confirm_key(&event, within, false, false));
            assert!(!should_guard_confirm_key(&event, within, true, true));
        }
        for code in [
            KeyCode::Up,
            KeyCode::Down,
            KeyCode::Esc,
            KeyCode::Char('n'),
            KeyCode::Char('2'),
            KeyCode::Char('3'),
        ] {
            let event = KeyEvent::new(code, KeyModifiers::NONE);
            assert!(!should_guard_confirm_key(&event, within, true, false));
        }
    }

    /// 【权限测试】【鼠标命中】验证选中与未选中权限行可解析对应选项且忽略行尾空白。
    /// 参数: 无；返回: 无。
    #[test]
    fn option_line_parser_matches_rendered_choices_within_bounds() {
        let rendered = crate::render::render_permission_controls(PermissionChoice::Allow, None);
        let lines: Vec<&str> = rendered.lines().collect();
        assert_eq!(
            parse_permission_option_line(lines[0], 4),
            Some(PermissionChoice::Allow)
        );
        assert_eq!(
            parse_permission_option_line(lines[1], 4),
            Some(PermissionChoice::Deny)
        );
        assert_eq!(
            parse_permission_option_line(lines[2], 6),
            Some(PermissionChoice::DenyWithReply)
        );
        assert_eq!(parse_permission_option_line(lines[1], 40), None);
    }
}
