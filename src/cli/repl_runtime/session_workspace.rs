//! 会话视图与输入状态按标识停放，终端资源只由当前运行期持有。

use super::*;
use std::collections::HashMap;

/// 将同一组会话字段集中定义为快照，并原子交换，避免遗漏队列或权限归属。
macro_rules! session_fields {
    ($($field:ident: $ty:ty = $initial:expr),* $(,)?) => {
        struct SessionSnapshot { $( $field: $ty, )* }
        impl SessionSnapshot {
            /// 【会话切换】【空白状态】无参数；返回未绑定终端资源的独立会话状态
            fn empty() -> Self { Self { $( $field: $initial, )* } }
            /// 【会话切换】【状态交换】参数 runtime 为前台运行期；返回无，交换整组会话状态
            fn exchange(&mut self, runtime: &mut ReplRuntime) {
                $( std::mem::swap(&mut self.$field, &mut runtime.$field); )*
            }
        }
    };
}

session_fields! {
    transcript: TranscriptStore = TranscriptStore::new(20_000),
    composer: Option<ComposerFrame> = None,
    composer_selection: composer_selection::ComposerSelection = Default::default(),
    stream_draft: StreamComposerDraft = Default::default(),
    submission_queue: Arc<Mutex<VecDeque<QueuedSubmission>>> = Arc::new(Mutex::new(VecDeque::new())),
    submission_queue_paused: Arc<AtomicBool> = Arc::new(AtomicBool::new(false)),
    control_queue: VecDeque<String> = VecDeque::new(),
    live_mode_handle: Option<Arc<crate::agent::LiveAgentMode>> = None,
    live_model_settings: Option<Arc<crate::agent::LiveModelSettings>> = None,
    live_session_id: Option<String> = None,
    permission_prompts: VecDeque<permission_prompt::PendingPermissionPrompt> = VecDeque::new(),
    last_chrome: Option<ReplChrome> = None,
    agent_panel: agent_panel::AgentPanelState = Default::default(),
    subagent_input_error: Option<(String, String)> = None,
    queue_panel: queue_panel::QueuePanelState = Default::default(),
    queue_draft_timing: QueueInsertAt = Default::default(),
    panels_dismissed: Option<(String, usize)> = None,
    todo_panel_compact: bool = true,
    stream_active: bool = false,
    live_usage: live_usage::LiveTurnUsage = Default::default(),
    pending_clear_queue: bool = false,
    ssh_prompt: Option<crate::cli::ssh_prompt::SshPromptView> = None,
    question_overlay_height: Option<u16> = None,
    next_live_refresh: Option<Instant> = None,
    live_sync_pending: bool = false,
    subagent_signature: Vec<(String, String, u64, u64)> = Vec::new(),
    background_logs: background_logs::BackgroundLogWatcher = Default::default(),
    background_panel: crate::cli::repl_background::EmbeddedTaskPanel = Default::default(),
}

struct ParkedSession {
    snapshot: SessionSnapshot,
    view: Option<fullscreen::SessionViewState>,
}

#[derive(Default)]
pub(super) struct SessionWorkspace {
    current: Option<String>,
    parked: HashMap<String, ParkedSession>,
    pub(super) requested: Option<String>,
    events: Option<std::sync::mpsc::Receiver<(String, crate::runner::RunnerEvent)>>,
    pub(super) sender: Option<std::sync::mpsc::Sender<(String, crate::runner::RunnerEvent)>>,
    pub(super) rendering_suspended: bool,
}

impl ReplRuntime {
    /// 【会话任务】【事件入口】无参数；返回按会话标记的发送端，不持有终端输入资源
    pub(in crate::cli) fn session_event_sender(
        &mut self,
    ) -> std::sync::mpsc::Sender<(String, crate::runner::RunnerEvent)> {
        if self.session_workspace.sender.is_none() {
            let (sender, events) = std::sync::mpsc::channel();
            self.session_workspace.sender = Some(sender);
            self.session_workspace.events = Some(events);
        }
        self.session_workspace.sender.as_ref().unwrap().clone()
    }

    /// 【会话任务】【后台投影】无参数；返回前台是否更新，后台事件只修改所属快照且禁止终端绘制
    pub(in crate::cli) fn tick_session_events(&mut self) -> Result<bool> {
        let mut changed = false;
        for _ in 0..512 {
            let next = self
                .session_workspace
                .events
                .as_ref()
                .and_then(|events| events.try_recv().ok());
            let Some((id, event)) = next else {
                break;
            };
            if self.session_workspace.current.as_deref() == Some(id.as_str()) {
                self.record_runner_event(&event)?;
                changed = true;
            } else if let Some(mut parked) = self.session_workspace.parked.remove(&id) {
                parked.snapshot.exchange(self);
                self.session_workspace.rendering_suspended = true;
                let result = self.record_runner_event(&event);
                self.session_workspace.rendering_suspended = false;
                parked.snapshot.exchange(self);
                self.session_workspace.parked.insert(id, parked);
                result?;
            }
        }
        Ok(changed)
    }

    /// 【会话切换】【请求状态】无参数；返回是否需要退出当前输入或流式前台循环
    pub(in crate::cli) fn has_session_switch_request(&self) -> bool {
        self.session_workspace.requested.is_some()
    }

    /// 【会话切换】【请求领取】无参数；返回目标会话标识，只有宿主可以真正切换执行归属
    pub(in crate::cli) fn take_session_switch_request(&mut self) -> Option<String> {
        self.session_workspace.requested.take()
    }

    /// 【会话切换】【草稿保存】参数 draft 为离开输入框前的完整草稿；返回无，保留附件及光标
    pub(in crate::cli) fn save_session_input_draft(&mut self, draft: StreamComposerDraft) {
        self.stream_draft = draft;
    }

    /// 【会话切换】【草稿领取】无参数；返回目标会话草稿，仅清空前台编辑副本
    pub(in crate::cli) fn take_session_input_draft(&mut self) -> StreamComposerDraft {
        std::mem::take(&mut self.stream_draft)
    }

    /// 【会话切换】【前台交接】参数 paths/state 为目标运行环境；返回目标是否已有缓存，首次会话由宿主重放历史
    pub(in crate::cli) fn switch_session_workspace(
        &mut self,
        paths: &crate::paths::SaiPaths,
        state: &crate::state::StateStore,
    ) -> Result<bool> {
        let id = state.session_id().to_string();
        if self.session_workspace.current.as_deref() == Some(&id) {
            return Ok(true);
        }
        self.commit_frame()?;
        // 1. 【会话切换】【旧会话停放】队列共享句柄随会话保存，后台 Agent 继续消费原队列
        if let Some(previous) = self.session_workspace.current.take() {
            let view = self.take_session_view();
            let mut snapshot = SessionSnapshot::empty();
            snapshot.exchange(self);
            self.session_workspace
                .parked
                .insert(previous, ParkedSession { snapshot, view });
        }
        // 2. 【会话切换】【目标恢复】只移动逻辑状态，备用屏和鼠标捕获不离开终端宿主
        let cached = if let Some(mut parked) = self.session_workspace.parked.remove(&id) {
            parked.snapshot.exchange(self);
            self.restore_session_view(parked.view);
            true
        } else {
            self.restore_session_view(None);
            false
        };
        self.session_workspace.current = Some(id.clone());
        self.session_workspace.requested = None;
        self.session_browser
            .bind(paths, &id, state.state_dir().to_path_buf());
        self.stream = StreamState::default();
        self.reflow = ReflowState::new();
        self.reflow
            .observe(TerminalSize::current(), self.stream_active);
        self.resize_preview = None;
        self.last_composer_signature = None;
        self.last_cursor_row = None;
        self.panel_pointer = None;
        self.completion_pointer = None;
        self.completion_clicks.clear();
        self.key_notice = None;
        self.force_reanchor = true;
        Ok(cached)
    }

    /// 【会话切换】【首次归属】参数 state 为初始会话；返回无，不停放尚未绑定的启动画面
    pub(in crate::cli) fn initialize_session_workspace(
        &mut self,
        state: &crate::state::StateStore,
    ) {
        self.session_workspace.current = Some(state.session_id().into());
    }

    /// 【会话切换】【底栏预热】提前注入目标会话状态栏快照，避免首帧渲染空指标后再触发二次重绘。
    /// 参数: `chrome` 为目标会话状态栏数据；返回: 无。
    pub(in crate::cli) fn prime_session_chrome(&mut self, chrome: ReplChrome) {
        self.last_chrome = Some(chrome);
    }

    /// 【会话切换】【全屏单帧批处理】全屏模式下挂起中间绘制步骤，全部数据就绪后执行一次原子差量重绘。
    /// 参数: `work` 为会话装载闭包；返回: 闭包执行结果。
    pub(in crate::cli) fn with_fullscreen_batch<T>(
        &mut self,
        work: impl FnOnce(&mut Self) -> Result<T>,
    ) -> Result<T> {
        if self.fullscreen.is_none() || self.session_workspace.rendering_suspended {
            return work(self);
        }
        // 1. 【会话切换】【挂起中间帧】阻止清屏、元信息写入与历史装载各自向终端提交半成品帧
        self.session_workspace.rendering_suspended = true;
        let outcome = work(self);
        self.session_workspace.rendering_suspended = false;
        let value = outcome?;
        // 2. 【会话切换】【原子提交】全部状态与历史就绪后一次性完成全屏差量合成
        self.paint_fullscreen()?;
        Ok(value)
    }
}
