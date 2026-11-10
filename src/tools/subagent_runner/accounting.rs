use super::{SubagentStats, TokenEstimateMethod};
use crate::llm::{ChatMessage, ChatResult};
use crate::state::estimate_chat_messages_tokens;

impl SubagentStats {
    /// 【子任务】【响应核算】每个成功响应仅在请求边界累计一次用量
    /// 参数：`messages` 为请求历史，`result` 为完整响应；返回：无
    pub(super) fn record_response(&mut self, messages: &[ChatMessage], result: &ChatResult) {
        if result
            .usage
            .as_ref()
            .is_some_and(|usage| usage.total_tokens > 0)
        {
            self.add_usage_or_estimate(result.usage.as_ref(), &[]);
            return;
        }
        // 1. 图片按视觉用量估算，避免把 base64 当作正文重复计数
        let estimated = estimate_chat_messages_tokens(messages)
            .saturating_add(estimate_chat_messages_tokens(&[result.assistant_message()]));
        self.token_estimate = self.token_estimate.saturating_add(estimated as u64);
        self.token_estimate_method = match self.token_estimate_method {
            TokenEstimateMethod::None | TokenEstimateMethod::RoughCharEstimate => {
                TokenEstimateMethod::RoughCharEstimate
            }
            _ => TokenEstimateMethod::ProviderUsagePlusEstimate,
        };
    }
}
