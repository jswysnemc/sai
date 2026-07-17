mod model;
mod policy;
mod repository;
pub(in crate::state) mod schema;
mod store;
pub(crate) mod summary;

use crate::state::turns::ConversationDb;
use anyhow::Result;

pub(crate) use model::NewRecoveryRecord;
pub use model::{FailureKind, RecoverySnapshot, RecoveryStatus};
pub(crate) use policy::AUTO_COMPACTION_FAILURE_THRESHOLD;

/// 写入恢复记录。
///
/// 参数:
/// - `db`: 对话数据库
/// - `record`: 待写入记录
///
/// 返回:
/// - 写入后的记录
pub(crate) fn record_failure(
    db: &ConversationDb,
    record: NewRecoveryRecord,
) -> Result<model::RecoveryRecord> {
    repository::insert_record(db, record)
}

/// 读取恢复快照。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
///
/// 返回:
/// - 恢复快照
pub(crate) fn recovery_snapshot(db: &ConversationDb, session_id: &str) -> Result<RecoverySnapshot> {
    repository::snapshot(db, session_id, AUTO_COMPACTION_FAILURE_THRESHOLD)
}

/// 判断是否允许自动压缩。
///
/// 参数:
/// - `snapshot`: 当前恢复快照
///
/// 返回:
/// - 是否允许自动压缩
pub(crate) fn should_attempt_auto_compaction(snapshot: &RecoverySnapshot) -> bool {
    policy::should_attempt_auto_compaction(snapshot)
}

/// 计算下一次自动压缩失败次数。
///
/// 参数:
/// - `snapshot`: 当前恢复快照
///
/// 返回:
/// - 下一次失败次数
pub(crate) fn next_auto_compaction_retry_count(snapshot: &RecoverySnapshot) -> usize {
    policy::next_auto_compaction_retry_count(snapshot)
}

/// 读取最近安全 checkpoint id。
///
/// 参数:
/// - `db`: 对话数据库
///
/// 返回:
/// - 最近 checkpoint id
pub(crate) fn latest_checkpoint_id(db: &ConversationDb) -> Result<Option<String>> {
    repository::latest_checkpoint_id(db)
}

/// 标记活跃压缩失败已恢复。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
///
/// 返回:
/// - 更新数量
pub(crate) fn resolve_active_compaction_failures(
    db: &ConversationDb,
    session_id: &str,
) -> Result<usize> {
    repository::resolve_active_compaction_failures(db, session_id)
}
