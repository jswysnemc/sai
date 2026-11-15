//! 会话切换只发送宿主请求，终端备用屏不随逻辑会话一起销毁。

use super::sidebar_sessions_view::key;
use super::state::FullscreenState;
use crate::cli::repl_runtime::ReplRuntime;

pub(in crate::cli::repl_runtime) struct SessionViewState {
    state: FullscreenState,
    viewing_subagent: Option<String>,
}

impl ReplRuntime {
    /// 【终端侧栏】【会话绑定】参数 paths/state 为实际前台会话；返回无，不根据共享指针猜测归属
    pub(in crate::cli) fn bind_session_browser(
        &mut self,
        paths: &crate::paths::SaiPaths,
        state: &crate::state::StateStore,
    ) {
        if self.session_browser.current.is_none() {
            self.initialize_session_workspace(state);
        }
        self.session_browser
            .bind(paths, state.session_id(), state.state_dir().to_path_buf());
    }

    /// 【终端侧栏】【直接切换】参数 action 为稳定条目键；返回无，宿主下一次调度立即切换并保留原轮次
    pub(super) fn browse_session(&mut self, action: u64) {
        if self
            .session_browser
            .current
            .as_ref()
            .is_some_and(|entry| key(entry) == action)
        {
            return;
        }
        if let Some(entry) = self
            .session_browser
            .entries
            .iter()
            .find(|entry| key(entry) == action)
        {
            self.session_workspace.requested = Some(entry.id.clone());
        }
    }

    /// 【终端侧栏】【目录刷新】无参数；返回会话目录是否变化，运行状态来自真实锁和当前执行状态
    pub(super) fn tick_session_browser(&mut self) -> bool {
        let visible = self.sidebar_visible(usize::from(
            super::super::viewport::TerminalSize::current().cols,
        ));
        self.session_browser.tick(visible)
    }

    /// 【会话切换】【阅读停放】无参数；返回逻辑阅读状态，不移动鼠标捕获和备用屏所有权
    pub(in crate::cli::repl_runtime) fn take_session_view(&mut self) -> Option<SessionViewState> {
        let session = self.fullscreen.as_mut()?;
        session.sidebar.preview.close();
        session.image_viewer.close();
        let state = std::mem::replace(&mut session.state, FullscreenState::new());
        Some(SessionViewState {
            state,
            viewing_subagent: session.viewing_subagent.take(),
        })
    }

    /// 【会话切换】【阅读恢复】参数 view 为目标会话阅读状态；返回无，清除旧会话所有命中和图像缓存
    pub(in crate::cli::repl_runtime) fn restore_session_view(
        &mut self,
        view: Option<SessionViewState>,
    ) {
        let Some(session) = self.fullscreen.as_mut() else {
            return;
        };
        let view = view.unwrap_or(SessionViewState {
            state: FullscreenState::new(),
            viewing_subagent: None,
        });
        session.state = view.state;
        session.state.previous = None;
        session.viewing_subagent = view.viewing_subagent;
        session.pending_toggle = None;
        session.pending_copy = None;
        session.images.clear();
        session.image_viewer.close();
        session.sidebar.preview.close();
        session.sidebar.hovered = None;
        session.sidebar.focused_row = None;
        session.sidebar.focused = false;
        session.pane_tail = None;
        session.bottom_button = None;
    }
}
