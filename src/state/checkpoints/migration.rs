use super::model::{CheckpointReason, CompactionCheckpoint};
use super::repository::{
    apply_legacy_checkpoint_migration, count_checkpoints, load_latest_checkpoint,
};
use crate::state::turns::Turn;
use crate::state::StateStore;
use anyhow::Result;
use chrono::Utc;

/// 从旧 JSON 压缩摘要迁移 checkpoint。
///
/// 参数:
/// - `store`: 当前状态存储
///
/// 返回:
/// - 是否创建了 legacy checkpoint
pub(in crate::state) fn migrate_legacy_compaction_summary(store: &StateStore) -> Result<bool> {
    let Some(summary) = store.load_compaction_summary()? else {
        return Ok(false);
    };
    if summary.compacted_turns == 0 || summary.summary.trim().is_empty() {
        return Ok(false);
    }
    {
        let conn = store.conv_db.conn.lock().unwrap();
        if count_checkpoints(&conn)? > 0 || load_latest_checkpoint(&conn)?.is_some() {
            return Ok(false);
        }
    }

    let turns = store.conv_db.load_turns()?;
    let boundary = legacy_boundary(&turns, summary.compacted_turns);
    let checkpoint = CompactionCheckpoint {
        id: format!(
            "cp_legacy_{}_{}",
            Utc::now().timestamp_millis(),
            rand::random::<u16>()
        ),
        seq: boundary.compacted_to_seq,
        compacted_from_seq: boundary.compacted_from_seq,
        compacted_to_seq: boundary.compacted_to_seq,
        summary: summary.summary.trim().to_string(),
        recent: boundary.recent,
        source_turn_count: summary.compacted_turns,
        reason: CheckpointReason::Legacy,
        created_at: summary.updated_at,
    };

    apply_legacy_checkpoint_migration(store.conv_db.as_ref(), &checkpoint, boundary.delete_to_seq)?;
    Ok(true)
}

struct LegacyBoundary {
    compacted_from_seq: i64,
    compacted_to_seq: i64,
    delete_to_seq: i64,
    recent: String,
}

/// 推断旧摘要覆盖的轮次边界。
///
/// 参数:
/// - `turns`: 当前 SQLite 中的原始轮次
/// - `compacted_turns`: 旧摘要记录的已压缩轮次数
///
/// 返回:
/// - legacy checkpoint 边界
fn legacy_boundary(turns: &[Turn], compacted_turns: usize) -> LegacyBoundary {
    if turns.is_empty() {
        return LegacyBoundary {
            compacted_from_seq: 0,
            compacted_to_seq: 0,
            delete_to_seq: 0,
            recent: String::new(),
        };
    }

    if turns.len() >= compacted_turns {
        let covered = &turns[..compacted_turns];
        return LegacyBoundary {
            compacted_from_seq: covered.first().map(|turn| turn.seq).unwrap_or_default(),
            compacted_to_seq: covered.last().map(|turn| turn.seq).unwrap_or_default(),
            delete_to_seq: covered.last().map(|turn| turn.seq).unwrap_or_default(),
            recent: recent_context(covered),
        };
    }

    let first_tail_seq = turns.first().map(|turn| turn.seq).unwrap_or_default();
    let compacted_to_seq = first_tail_seq.saturating_sub(1);
    let compacted_from_seq = compacted_to_seq
        .saturating_sub(compacted_turns as i64)
        .saturating_add(1)
        .max(0);
    LegacyBoundary {
        compacted_from_seq,
        compacted_to_seq,
        delete_to_seq: 0,
        recent: String::new(),
    }
}

/// 构造 legacy checkpoint 的最近上下文。
///
/// 参数:
/// - `turns`: 旧摘要覆盖的原始轮次
///
/// 返回:
/// - 最近上下文文本
fn recent_context(turns: &[Turn]) -> String {
    turns
        .iter()
        .rev()
        .take(2)
        .rev()
        .map(|turn| {
            format!(
                "User: {}\nAssistant: {}",
                turn.user_content, turn.assistant_content
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::legacy_boundary;
    use crate::state::turns::{Turn, TurnStatus};

    fn turn(seq: i64) -> Turn {
        Turn {
            turn_id: format!("turn_{seq}"),
            seq,
            user_content: format!("user {seq}"),
            user_image_urls: Vec::new(),
            user_timestamp: "2026-01-01T00:00:00Z".to_string(),
            assistant_content: format!("assistant {seq}"),
            assistant_reasoning: None,
            assistant_timestamp: Some("2026-01-01T00:00:01Z".to_string()),
            status: TurnStatus::Completed,
            tool_reports: Vec::new(),
        }
    }

    #[test]
    fn boundary_deletes_covered_raw_turns_when_they_still_exist() {
        let boundary = legacy_boundary(&[turn(1), turn(2), turn(3)], 2);

        assert_eq!(boundary.compacted_from_seq, 1);
        assert_eq!(boundary.compacted_to_seq, 2);
        assert_eq!(boundary.delete_to_seq, 2);
        assert!(boundary.recent.contains("assistant 2"));
    }

    #[test]
    fn boundary_preserves_tail_when_old_raw_turns_are_already_missing() {
        let boundary = legacy_boundary(&[turn(5), turn(6)], 4);

        assert_eq!(boundary.compacted_from_seq, 1);
        assert_eq!(boundary.compacted_to_seq, 4);
        assert_eq!(boundary.delete_to_seq, 0);
        assert!(boundary.recent.is_empty());
    }
}
