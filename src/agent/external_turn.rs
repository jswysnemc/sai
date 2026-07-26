use super::{Agent, AgentEvent};
use crate::agent_engine::TurnRequest;
use crate::llm::ChatResult;
use crate::state::PendingTurnGuard;
use anyhow::Result;

impl Agent {
    /// 判断本轮是否交给外部内核执行。
    ///
    /// 返回:
    /// - 配置了可用的外部内核时为 true
    pub(super) fn uses_external_engine(&self) -> bool {
        self.external_engine.is_some()
    }

    /// 用外部内核执行一轮对话。
    ///
    /// 保留 sai 的治理与持久化外壳——轮次记录、未完成轮守卫、工作树撤销点，
    /// 但跳过消息组装、上下文压缩与记忆注入：外部内核自己维护对话历史，
    /// sai 拼装的消息对它没有意义（这一点在切换内核时会明确提示用户）。
    ///
    /// 参数:
    /// - `input`: 用户输入
    /// - `image_urls`: 随本轮提交的图片
    /// - `turn_id`: 轮次标识
    /// - `on_event`: 流式事件回调
    ///
    /// 返回:
    /// - 本轮结果
    pub(super) async fn run_external_turn<F>(
        &mut self,
        input: &str,
        image_urls: Vec<String>,
        turn_id: String,
        mut on_event: F,
    ) -> Result<ChatResult>
    where
        F: FnMut(AgentEvent) -> Result<()>,
    {
        // 1. 与原生路径一致地登记轮次，历史与时间线因此不区分内核
        self.state
            .start_turn_with_images(&turn_id, input, &image_urls)?;
        let guard = PendingTurnGuard::new(self.state.clone(), turn_id.clone());
        let cwd = crate::runtime_cwd::current_dir()?;
        let worktree_undo =
            crate::state::worktree_undo::WorktreeUndoGuard::begin(&self.state, &cwd, &turn_id)?;
        let request = TurnRequest {
            input: input.to_string(),
            image_urls,
            cwd,
        };
        // 2. 交给外部内核跑完一轮。内核只拿到通道发送端，
        //    回调留在本函数里驱动：它借自调用方栈上的闭包，不保证能跨线程移动，
        //    而整个对话 future 在网关路径上会被 tokio::spawn
        let engine = self
            .external_engine
            .as_mut()
            .expect("external engine must exist when uses_external_engine() is true");
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let mut turn = Box::pin(engine.run_turn(request, sender));
        let result = loop {
            tokio::select! {
                // 优先派发事件，保证 UI 的更新顺序与内核产出顺序一致
                biased;
                Some(event) = receiver.recv() => on_event(event)?,
                outcome = &mut turn => break outcome,
            }
        };
        // 3. 内核已返回，把仍在通道里的尾部事件排空后再收尾
        while let Ok(event) = receiver.try_recv() {
            on_event(event)?;
        }
        match result {
            Ok(result) => {
                // 4. 助手回复写回会话，恢复历史时与原生内核同构
                guard.complete(&result.content, result.reasoning.as_deref())?;
                worktree_undo.finish()?;
                Ok(result)
            }
            Err(error) => {
                let _ = guard.fail(&error.to_string());
                Err(error)
            }
        }
    }
}
