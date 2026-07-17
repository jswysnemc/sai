use super::model::ContextEpochSummary;
use super::repository;
use crate::state::turns::ConversationDb;
use anyhow::Result;

/// 读取 Context Epoch 摘要。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 当前会话标识
///
/// 返回:
/// - Context Epoch 摘要
pub(crate) fn context_epoch_summary(
    db: &ConversationDb,
    session_id: &str,
) -> Result<Option<ContextEpochSummary>> {
    repository::load_summary(db, session_id)
}
