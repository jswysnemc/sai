use super::StateStore;
use crate::llm::ChatStreamKind;
use anyhow::Result;

pub struct PendingTurnGuard {
    state: StateStore,
    turn_id: String,
    completed: bool,
    partial_content: String,
    partial_reasoning: String,
}

impl PendingTurnGuard {
    /// 创建待完成轮次守卫。
    ///
    /// 参数:
    /// - `state`: 状态存储
    /// - `turn_id`: 当前轮唯一标识
    ///
    /// 返回:
    /// - 待完成轮次守卫
    pub fn new(state: StateStore, turn_id: String) -> Self {
        Self {
            state,
            turn_id,
            completed: false,
            partial_content: String::new(),
            partial_reasoning: String::new(),
        }
    }

    /// 累积已经发送给用户的流式助手内容。
    ///
    /// 参数:
    /// - `kind`: 流式内容类型
    /// - `text`: 本次增量文本
    ///
    /// 返回:
    /// - 无
    pub fn append_chunk(&mut self, kind: ChatStreamKind, text: &str) {
        match kind {
            ChatStreamKind::Content => self.partial_content.push_str(text),
            ChatStreamKind::Reasoning => self.partial_reasoning.push_str(text),
        }
    }

    /// 完成当前轮次并关闭守卫。
    ///
    /// 参数:
    /// - `content`: 助手回复
    /// - `reasoning`: 可选推理内容
    ///
    /// 返回:
    /// - 完成是否成功
    pub fn complete(mut self, content: &str, reasoning: Option<&str>) -> Result<()> {
        self.state
            .complete_turn(&self.turn_id, content, reasoning)?;
        self.completed = true;
        Ok(())
    }

    /// 手动中断当前轮次。
    ///
    /// 返回:
    /// - 中断是否成功
    #[allow(dead_code)]
    pub fn interrupt(&mut self) -> Result<()> {
        if !self.completed {
            self.persist_interruption()?;
            self.completed = true;
        }
        Ok(())
    }
}

impl PendingTurnGuard {
    /// 将当前轮次和已经产生的工具历史保存为中断状态。
    ///
    /// 返回:
    /// - 写入是否成功
    fn persist_interruption(&self) -> Result<()> {
        self.state.interrupt_turn(
            &self.turn_id,
            &self.partial_content,
            (!self.partial_reasoning.trim().is_empty()).then_some(self.partial_reasoning.as_str()),
        )
    }
}

impl Drop for PendingTurnGuard {
    fn drop(&mut self) {
        if !self.completed {
            let _ = self.persist_interruption();
        }
    }
}
