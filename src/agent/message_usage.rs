use super::Agent;
use crate::llm::{ChatResult, Usage};
use anyhow::Result;

impl Agent {
    /// 保存一次主对话模型消息的用量和全局记录。
    ///
    /// 参数:
    /// - `result`: provider 返回结果
    /// 返回:
    /// - 持久化是否成功
    pub(super) fn record_message_usage(&self, result: &ChatResult) -> Result<()> {
        let started_at = chrono::Utc::now().timestamp();
        match result.usage.as_ref() {
            Some(usage) => {
                self.state.add_conversation_message_usage(usage)?;
                let _ = crate::usage_history::record_model_call(
                    &self.paths,
                    crate::usage_history::UsageRecordInput {
                        provider_id: self.client.provider_id(),
                        provider_name: self.client.provider_name(),
                        model: self.client.model(),
                        source: "chat",
                        operation: "message",
                        status: "success",
                        usage: Some(usage),
                        usage_source: "provider_reported",
                        started_at,
                        duration_ms: result.duration_ms,
                        session_id: Some(self.state.session_id()),
                        error_kind: None,
                    },
                );
            }
            None => {
                self.state.clear_last_conversation_usage()?;
                let _ = crate::usage_history::record_model_call(
                    &self.paths,
                    crate::usage_history::UsageRecordInput {
                        provider_id: self.client.provider_id(),
                        provider_name: self.client.provider_name(),
                        model: self.client.model(),
                        source: "chat",
                        operation: "message",
                        status: "success",
                        usage: None,
                        usage_source: "missing",
                        started_at,
                        duration_ms: result.duration_ms,
                        session_id: Some(self.state.session_id()),
                        error_kind: None,
                    },
                );
            }
        }
        Ok(())
    }
}

/// 把一次模型请求用量累加到当前完整轮次。
///
/// 参数:
/// - `total`: 当前轮已累计用量
/// - `usage`: 新完成的模型请求用量
///
/// 返回:
/// - 无
pub(super) fn accumulate_turn_usage(total: &mut Option<Usage>, usage: Option<&Usage>) {
    let Some(usage) = usage else {
        return;
    };
    let total = total.get_or_insert_with(Usage::default);
    total.prompt_tokens = total.prompt_tokens.saturating_add(usage.prompt_tokens);
    total.completion_tokens = total
        .completion_tokens
        .saturating_add(usage.completion_tokens);
    total.total_tokens = total.total_tokens.saturating_add(usage.total_tokens);
    total.cache_read_tokens = total
        .cache_read_tokens
        .saturating_add(usage.cache_read_tokens);
    total.cache_write_tokens = total
        .cache_write_tokens
        .saturating_add(usage.cache_write_tokens);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证工具多轮请求的 token 与缓存数据按完整轮次累加。
    #[test]
    fn accumulates_all_turn_usage_fields() {
        let mut total = None;
        accumulate_turn_usage(
            &mut total,
            Some(&Usage {
                prompt_tokens: 100,
                completion_tokens: 20,
                total_tokens: 120,
                cache_read_tokens: 80,
                cache_write_tokens: 5,
            }),
        );
        accumulate_turn_usage(
            &mut total,
            Some(&Usage {
                prompt_tokens: 50,
                completion_tokens: 10,
                total_tokens: 60,
                cache_read_tokens: 40,
                cache_write_tokens: 2,
            }),
        );

        let total = total.unwrap();
        assert_eq!(total.prompt_tokens, 150);
        assert_eq!(total.completion_tokens, 30);
        assert_eq!(total.total_tokens, 180);
        assert_eq!(total.cache_read_tokens, 120);
        assert_eq!(total.cache_write_tokens, 7);
    }
}
