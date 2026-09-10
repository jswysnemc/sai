use crate::llm::ChatStreamEvent;
use anyhow::{bail, Result};
use std::collections::BTreeMap;

/// 【插件模型】【流式预算】正文、思考和工具参数共同计量，供应商未结束连接也能拒绝超限响应。
pub(super) struct StreamBudget {
    limit: usize,
    received: usize,
    tool_bytes: BTreeMap<usize, usize>,
}

impl StreamBudget {
    /// 【插件模型】【预算创建】创建一次请求独立的输出计数器。
    /// @param limit 允许的全部流式输出字节
    /// @returns 尚未消费的预算
    pub(super) fn new(limit: usize) -> Self {
        Self {
            limit,
            received: 0,
            tool_bytes: BTreeMap::new(),
        }
    }

    /// 【插件模型】【流量检查】按实际新增字节计量，累计工具参数只计算增量。
    /// @param event 已收到的正文、思考或工具进度
    /// @returns 未超过上限时成功
    pub(super) fn observe(&mut self, event: &ChatStreamEvent) -> Result<()> {
        let added = match event {
            ChatStreamEvent::Chunk(chunk) => chunk.text.len(),
            ChatStreamEvent::ToolCallProgress(progress) => {
                let bytes = progress
                    .arguments_bytes
                    .saturating_add(progress.name.as_ref().map_or(0, String::len));
                bytes.saturating_sub(self.tool_bytes.insert(progress.index, bytes).unwrap_or(0))
            }
        };
        self.received = self.received.saturating_add(added);
        if self.received > self.limit {
            bail!("plugin model response exceeds size limit");
        }
        Ok(())
    }
}
