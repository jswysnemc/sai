use super::StateStore;
use anyhow::Result;

impl StateStore {
    /// 保存仅供 provider 重放的用户消息。
    ///
    /// 参数:
    /// - `turn_id`: 当前轮唯一标识
    /// - `content`: 状态事件与原始输入合并后的消息
    ///
    /// 返回:
    /// - 保存结果
    pub(crate) fn set_provider_user_content(&self, turn_id: &str, content: &str) -> Result<()> {
        self.conv_db.set_provider_user_content(turn_id, content)
    }
}
