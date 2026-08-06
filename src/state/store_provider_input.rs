use super::StateStore;
use anyhow::Result;

impl StateStore {
    /// 保存当前轮供应商实际接收的用户消息。
    ///
    /// 参数:
    /// - `turn_id`: 当前轮唯一标识
    /// - `content`: 合并内部上下文后的用户消息
    ///
    /// 返回:
    /// - 保存是否成功
    pub(crate) fn set_provider_user_content(&self, turn_id: &str, content: &str) -> Result<()> {
        self.conv_db.set_provider_user_content(turn_id, content)
    }
}
