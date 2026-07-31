use super::usage::{self, UsageSnapshot};
use super::StateStore;
use crate::llm::Usage;
use anyhow::Result;

impl StateStore {
    /// 累加一个完整轮次的用量。
    ///
    /// 参数:
    /// - `turn_usage`: 本轮全部模型调用的用量累计
    /// - `context_usage`: 最后一次调用的用量；未上报时为 None
    ///
    /// 返回:
    /// - 写入是否成功
    pub fn add_turn_usage(&self, turn_usage: &Usage, context_usage: Option<&Usage>) -> Result<()> {
        self.init_files()?;
        usage::add_turn_usage(&self.usage_file(), turn_usage, context_usage)
    }

    /// 累加辅助模型用量。
    ///
    /// 参数:
    /// - `usage`: 辅助模型用量
    ///
    /// 返回:
    /// - 写入是否成功
    pub fn add_auxiliary_usage(&self, usage: &Usage) -> Result<()> {
        self.init_files()?;
        usage::add_auxiliary_usage(&self.usage_file(), usage)
    }

    /// 读取累计用量快照。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 累计用量快照
    pub fn usage_snapshot(&self) -> Result<UsageSnapshot> {
        usage::snapshot(&self.usage_file())
    }

    /// 清空最近一次主对话 provider usage，保留累计统计。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 清空是否成功
    pub(crate) fn clear_last_conversation_usage(&self) -> Result<()> {
        usage::clear_last_conversation_usage(&self.usage_file())
    }

    /// 清空最近一次 provider usage。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 清空是否成功
    pub(super) fn clear_last_usage(&self) -> Result<()> {
        usage::clear_last_usage(&self.usage_file())
    }
}
