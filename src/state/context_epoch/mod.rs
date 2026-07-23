pub(crate) mod model;
pub(crate) mod repository;
pub(crate) mod schema;
pub(crate) mod snapshot;
pub(crate) mod source;
pub(crate) mod summary;

use super::turns::ConversationDb;
use anyhow::{bail, Result};
use model::ContextEpoch;

pub use model::{ContextEpochProjection, ContextEpochSummary, ContextSourceInput};

/// 准备当前会话的 Context Epoch baseline。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 当前会话标识
/// - `system_prompt`: 当前稳定系统提示
///
/// 返回:
/// - 最新 Context Epoch
pub(crate) fn prepare_context_epoch(
    db: &ConversationDb,
    session_id: &str,
    system_prompt: &str,
) -> Result<ContextEpoch> {
    let sources = source::stable_sources_from_prompt(system_prompt);
    let baseline = source::stable_baseline(system_prompt);
    let snapshot_json = snapshot::snapshot_json(&sources)?;
    let baseline_hash = snapshot::baseline_hash(&baseline);
    repository::prepare_epoch(
        db,
        session_id,
        repository::PreparedEpoch {
            baseline,
            baseline_hash,
            snapshot_json,
            source_count: sources.len(),
            blocked_source: None,
        },
    )
}

/// 读取当前会话 Context Epoch 摘要。
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
    summary::context_epoch_summary(db, session_id)
}

/// 构造当前会话 Context Epoch 投影。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 当前会话标识
/// - `system_prompt`: 当前稳定系统提示
///
/// 返回:
/// - Context Epoch 投影
pub(crate) fn context_epoch_projection(
    db: &ConversationDb,
    session_id: &str,
    system_prompt: &str,
) -> Result<model::ContextEpochProjection> {
    context_epoch_projection_from_sources(
        db,
        session_id,
        vec![source::source_input_from_prompt(system_prompt)],
    )
}

/// 从 Context Source 输入构造当前会话 Context Epoch 投影。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 当前会话标识
/// - `sources`: Context Source 输入集合
///
/// 返回:
/// - Context Epoch 投影
pub(crate) fn context_epoch_projection_from_sources(
    db: &ConversationDb,
    session_id: &str,
    sources: Vec<ContextSourceInput>,
) -> Result<model::ContextEpochProjection> {
    source::validate_unique_keys(&sources)?;
    let epoch = if let Some(blocked_source) = source::blocked_source(&sources) {
        match repository::mark_blocked_source(db, session_id, blocked_source.clone())? {
            Some(epoch) => epoch,
            None => {
                bail!("Context Epoch source blocked without existing baseline: {blocked_source}")
            }
        }
    } else {
        let source_snapshots = source::stable_sources_from_inputs(&sources);
        if source_snapshots.is_empty() {
            bail!("Context Epoch projection has no available sources");
        }
        let baseline = source::stable_baseline_from_inputs(&sources);
        let snapshot_json = snapshot::snapshot_json(&source_snapshots)?;
        let baseline_hash = snapshot::baseline_hash(&baseline);
        repository::prepare_epoch(
            db,
            session_id,
            repository::PreparedEpoch {
                baseline,
                baseline_hash,
                snapshot_json,
                source_count: source_snapshots.len(),
                blocked_source: None,
            },
        )?
    };
    Ok(ContextEpochProjection {
        baseline: epoch.baseline,
        baseline_hash: epoch.baseline_hash,
        source_count: epoch.source_count,
        last_change_reason: model::reason_to_str(&epoch.last_change_reason).to_string(),
        blocked_source: epoch.blocked_source,
    })
}

/// 读取当前会话已持久化的 Context Epoch baseline 文本。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 当前会话标识
///
/// 返回:
/// - baseline 文本；尚未初始化时返回 None
pub(crate) fn load_baseline(db: &ConversationDb, session_id: &str) -> Result<Option<String>> {
    Ok(repository::load_epoch(db, session_id)?.map(|epoch| epoch.baseline))
}

