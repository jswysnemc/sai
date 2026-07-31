use super::{
    context_epoch, ContextEpochProjection, ContextEpochSummary, ContextSourceInput, StateStore,
};
use anyhow::Result;

impl StateStore {
    /// 读取当前会话 Context Epoch 摘要。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - Context Epoch 摘要
    pub fn context_epoch_summary(&self) -> Result<Option<ContextEpochSummary>> {
        context_epoch::context_epoch_summary(&self.conv_db, &self.session_id)
    }

    /// 读取当前会话 Context Epoch baseline 文本。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - baseline 文本；尚未初始化时返回 None
    pub fn context_epoch_baseline(&self) -> Result<Option<String>> {
        context_epoch::load_baseline(&self.conv_db, &self.session_id)
    }

    /// 构造当前会话 Context Epoch 投影。
    ///
    /// 参数:
    /// - `system_prompt`: 当前稳定系统提示
    ///
    /// 返回:
    /// - Context Epoch 投影
    pub fn context_epoch_projection(&self, system_prompt: &str) -> Result<ContextEpochProjection> {
        let result =
            context_epoch::context_epoch_projection(&self.conv_db, &self.session_id, system_prompt);
        self.record_context_epoch_projection_result(&result)?;
        result
    }

    /// 从 Context Source 输入构造当前会话 Context Epoch 投影。
    ///
    /// 参数:
    /// - `sources`: Context Source 输入集合
    ///
    /// 返回:
    /// - Context Epoch 投影
    #[allow(dead_code)]
    pub fn context_epoch_projection_from_sources(
        &self,
        sources: Vec<ContextSourceInput>,
    ) -> Result<ContextEpochProjection> {
        let result = context_epoch::context_epoch_projection_from_sources(
            &self.conv_db,
            &self.session_id,
            sources,
        );
        self.record_context_epoch_projection_result(&result)?;
        result
    }
}
