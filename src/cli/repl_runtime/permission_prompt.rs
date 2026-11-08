use super::ReplRuntime;
use crate::agent::{AgentMode, PermissionMode};
use crate::permission::{
    self, PermissionDecision, PermissionInteractionState, PermissionRequest, PermissionTransition,
};
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};

pub(super) struct PendingPermissionPrompt {
    request: PermissionRequest,
    state: PermissionInteractionState,
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
                if crossterm::event::poll(std::time::Duration::ZERO)? {
                    self.handle_permission_input(&crossterm::event::read()?)?;
                } else {
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
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
        self.pause_for_permission_prompt()?;
        self.record_permission_request(request.clone())?;
        self.permission_prompts.push_back(PendingPermissionPrompt {
            request,
            state: PermissionInteractionState::new(),
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

    /// 【权限】【事件处理】参数 input 为已就绪事件；返回是否由审批消费，不进行阻塞读取。
    pub(super) fn handle_permission_input(&mut self, input: &Event) -> Result<bool> {
        self.refresh_permission_prompt()?;
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
        // 2. 【权限】【输入分派】尺寸与滚动仍由终端处理，其余输入只进入当前审批
        if let Event::Resize(cols, rows) = input {
            self.observe_stream_resize(*cols, *rows)?;
        } else if matches!(input, Event::Mouse(_)) {
            self.handle_fullscreen_event(input)?;
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
