use super::{Agent, AgentEvent};
use crate::goal::Goal;
use crate::llm::{ChatMessage, ChatResult, Usage};
use crate::perf_trace::PerfTrace;
use anyhow::Result;

impl Agent {
    /// 【目标】【续作条件】规划模式或目标工具不可用时不启动自动执行。
    /// 参数：无；返回当前运行环境是否支持目标完成与状态更新。
    pub(crate) fn goal_execution_available(&self) -> bool {
        self.mode().permission() != super::PermissionMode::Plan
            && self.tools_enabled
            && self.tools.contains("update_goal")
    }

    /// 【目标】【执行边界】读取当前运行中已停止的目标，普通无目标对话保持独立
    /// 参数：无；返回需要收尾的目标
    pub(super) fn stopped_goal(&self) -> Result<Option<Goal>> {
        match &self.goal_accounting {
            Some(accounting) => Ok(accounting
                .current_goal()?
                .filter(|goal| !goal.status.is_active())),
            None => Ok(None),
        }
    }

    /// 【目标】【模型收尾】停止后仅允许一次无工具总结，保留准确用量和最终历史
    /// 参数：轮次、目标、历史、累计用量、事件和计时器；返回最终响应
    pub(super) async fn finalize_stopped_goal<F>(
        &mut self,
        turn_id: &str,
        goal: &Goal,
        messages: &mut Vec<ChatMessage>,
        usage: &mut Option<Usage>,
        on_event: &mut F,
        perf: &mut PerfTrace,
    ) -> Result<ChatResult>
    where
        F: FnMut(AgentEvent) -> Result<()>,
    {
        messages.push(ChatMessage::plain("user", crate::goal::stop_prompt(goal)));
        let mut result = self
            .request_model_round(
                turn_id,
                messages.clone(),
                Vec::new(),
                usize::MAX,
                on_event,
                perf,
            )
            .await?;
        self.record_message_usage(&result)?;
        super::message_usage::accumulate_turn_usage(usage, result.usage.as_ref());
        anyhow::ensure!(
            result.tool_calls.is_empty(),
            "goal finalization requested tools after execution stopped"
        );
        self.state.save_assistant_message(
            turn_id,
            crate::state::tool_history::AssistantMessageKey::Final,
            &result,
        )?;
        result.usage = usage.clone();
        Ok(result)
    }
}
