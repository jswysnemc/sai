use super::ReplRuntime;

impl ReplRuntime {
    /// 【会话模式】【界面绑定】参数 handle 为共享状态、session_id 为会话标识；绑定后返回无。
    pub(in crate::cli) fn bind_live_mode(
        &mut self,
        handle: std::sync::Arc<crate::agent::LiveAgentMode>,
        session_id: impl Into<String>,
    ) {
        self.live_mode_handle = Some(handle);
        self.live_session_id = Some(session_id.into());
    }

    /// 【会话模式】【界面解绑】无参数；轮次结束时清除共享状态，返回无。
    pub(in crate::cli) fn clear_live_mode(&mut self) {
        self.live_mode_handle = None;
        self.live_session_id = None;
    }

    /// 【会话模式】【实时应用】参数 fallback 为缺省模式；应用草稿组合并返回实际模式。
    pub(in crate::cli) fn apply_stream_mode_live(
        &mut self,
        fallback: crate::agent::AgentMode,
    ) -> crate::agent::AgentMode {
        let mode = self.stream_draft.mode.unwrap_or(fallback);
        if let Some(handle) = self.live_mode_handle.as_ref() {
            handle.set(mode);
        }
        if mode.permission() == crate::agent::PermissionMode::Yolo {
            if let Some(session_id) = self.live_session_id.as_deref() {
                let _ = crate::permission::allow_all_pending_for_session(session_id);
            }
        }
        mode
    }
}
