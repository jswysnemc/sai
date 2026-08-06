use super::Agent;
use crate::llm::ChatResult;
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
