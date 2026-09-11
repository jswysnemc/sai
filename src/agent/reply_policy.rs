use super::{Agent, AgentEvent, AgentMode};
use crate::tools::PreparedPluginReplies;
use anyhow::Result;
use std::sync::atomic::Ordering;

impl Agent {
    /// 【回复策略】【本轮准备】在主请求前异步读取插件上下文，同步投影只使用本次快照
    /// @param input 当前用户输入
    /// @returns 当前轮的提醒和不可复用投递资料
    pub(super) async fn prepare_reply_policies(&mut self, input: &str) -> PreparedPluginReplies {
        let mut prepared = self
            .tools
            .prepare_plugin_replies(input, self.mode() != AgentMode::Plan)
            .await;
        self.plugin_reply_contexts = std::mem::take(&mut prepared.contexts);
        prepared
    }

    /// 【回复策略】【回复后投递】主回复已经完成，策略和外部输出错误只记日志
    /// @param prepared 原轮次资料；emit 为交互面输出回调
    /// @returns 无；取消请求、计划模式及失效实例不会继续投递
    pub(super) async fn complete_reply_policies<F>(
        &mut self,
        prepared: PreparedPluginReplies,
        emit: &mut F,
    ) where
        F: FnMut(AgentEvent) -> Result<()>,
    {
        if !prepared.has_delivery()
            || self.mode() == AgentMode::Plan
            || self.cancel_requested.load(Ordering::SeqCst)
        {
            return;
        }
        if let Err(error) = emit(AgentEvent::ExternalOutput) {
            eprintln!("【回复策略】【输出失败】{error:#}");
            return;
        }
        for (id, (instance, text)) in self
            .tools
            .complete_plugin_replies(prepared, self.mode() != AgentMode::Plan)
            .await
        {
            if let Some(text) = text {
                self.plugin_reply_contexts.insert(id, (instance, text));
            } else {
                self.plugin_reply_contexts.remove(&id);
            }
        }
    }
}
