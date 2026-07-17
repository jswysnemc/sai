use super::model::{NewSessionMemory, SessionMemory};
use super::repository::{load_memory, record_failure as record_memory_failure, upsert_memory};
use crate::state::failure_recovery::{self, FailureKind, NewRecoveryRecord, RecoveryStatus};
use crate::state::turns::{ConversationDb, Turn, TurnStatus};
use anyhow::Result;
use chrono::{DateTime, Utc};

/// Session Memory 提取配置。
#[derive(Debug, Clone)]
pub(crate) struct ExtractionConfig {
    pub min_new_turns: usize,
    pub failure_threshold: usize,
    pub disable_seconds: i64,
}

impl ExtractionConfig {
    /// 返回默认提取配置。
    ///
    /// 返回:
    /// - 提取配置
    pub(crate) fn default_after_turn() -> Self {
        Self {
            min_new_turns: 1,
            failure_threshold: 3,
            disable_seconds: 3_600,
        }
    }

    /// 返回每次完成轮次都尝试提取的配置。
    ///
    /// 返回:
    /// - 提取配置
    #[cfg(test)]
    fn always() -> Self {
        Self {
            min_new_turns: 1,
            failure_threshold: 3,
            disable_seconds: 3_600,
        }
    }
}

/// Session Memory 提取结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExtractionOutcome {
    pub extracted: bool,
    pub skipped_reason: Option<String>,
}

/// 完成轮次后尝试提取 Session Memory。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `config`: 提取配置
/// - `summarize`: 摘要生成函数
///
/// 返回:
/// - 提取结果
pub(crate) fn extract_after_turn<F>(
    db: &ConversationDb,
    session_id: &str,
    config: ExtractionConfig,
    summarize: F,
) -> Result<ExtractionOutcome>
where
    F: FnOnce(&[Turn], Option<&SessionMemory>) -> Result<String>,
{
    let previous = load_memory(db, session_id)?;
    if is_disabled(previous.as_ref()) {
        return Ok(ExtractionOutcome {
            extracted: false,
            skipped_reason: Some("session memory extraction is disabled".to_string()),
        });
    }
    let after_seq = previous
        .as_ref()
        .map(|memory| memory.last_summarized_seq)
        .unwrap_or_default();
    let turns = db
        .load_turns_after_seq(after_seq, None)?
        .into_iter()
        .filter(|turn| turn.status != TurnStatus::Running)
        .collect::<Vec<_>>();
    if turns.len() < config.min_new_turns {
        return Ok(ExtractionOutcome {
            extracted: false,
            skipped_reason: Some("not enough completed turns".to_string()),
        });
    }
    let summary = match summarize(&turns, previous.as_ref()) {
        Ok(summary) if !summary.trim().is_empty() => summary.trim().to_string(),
        Ok(_) => {
            record_extraction_failure(
                db,
                session_id,
                "session memory summary is empty",
                &turns,
                config.failure_threshold,
                config.disable_seconds,
            )?;
            anyhow::bail!("session memory summary is empty");
        }
        Err(error) => {
            record_extraction_failure(
                db,
                session_id,
                &error.to_string(),
                &turns,
                config.failure_threshold,
                config.disable_seconds,
            )?;
            return Err(error);
        }
    };
    let last_turn = turns
        .last()
        .ok_or_else(|| anyhow::anyhow!("session memory extraction has no source turns"))?;
    upsert_memory(
        db,
        NewSessionMemory {
            session_id: session_id.to_string(),
            summary,
            last_summarized_turn_id: Some(last_turn.turn_id.clone()),
            last_summarized_seq: last_turn.seq,
            checkpoint_id: crate::state::failure_recovery::latest_checkpoint_id(db)?,
            source_turn_count: previous
                .as_ref()
                .map(|memory| memory.source_turn_count)
                .unwrap_or_default()
                + turns.len(),
            token_estimate: estimate_turn_tokens(&turns),
        },
    )?;
    Ok(ExtractionOutcome {
        extracted: true,
        skipped_reason: None,
    })
}

/// 记录 Session Memory 提取失败。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `reason`: 失败原因
/// - `turns`: 参与本次提取的来源轮次
/// - `failure_threshold`: 触发熔断的连续失败次数
/// - `disable_seconds`: 熔断持续秒数
///
/// 返回:
/// - 记录是否成功
pub(super) fn record_extraction_failure(
    db: &ConversationDb,
    session_id: &str,
    reason: &str,
    turns: &[Turn],
    failure_threshold: usize,
    disable_seconds: i64,
) -> Result<()> {
    let memory = record_memory_failure(db, session_id, reason, failure_threshold, disable_seconds)?;
    failure_recovery::record_failure(
        db,
        NewRecoveryRecord {
            session_id: session_id.to_string(),
            turn_id: turns.last().map(|turn| turn.turn_id.clone()),
            kind: FailureKind::SessionMemoryExtractionFailed,
            status: RecoveryStatus::Observed,
            reason: reason.to_string(),
            retry_count: memory.consecutive_failures,
            checkpoint_id: failure_recovery::latest_checkpoint_id(db)?,
            context_chars: estimate_turn_chars(turns),
            context_limit_chars: 0,
        },
    )?;
    Ok(())
}

/// 使用默认摘要器提取 Session Memory。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
///
/// 返回:
/// - 提取结果
pub(crate) fn extract_after_turn_with_default_summary(
    db: &ConversationDb,
    session_id: &str,
) -> Result<ExtractionOutcome> {
    extract_after_turn(
        db,
        session_id,
        ExtractionConfig::default_after_turn(),
        |turns, previous| Ok(default_summary(turns, previous)),
    )
}

/// 判断提取是否处于熔断期。
///
/// 参数:
/// - `memory`: 当前会话工作记忆
///
/// 返回:
/// - 是否处于熔断期
pub(super) fn is_disabled(memory: Option<&SessionMemory>) -> bool {
    let Some(disabled_until) = memory.and_then(|memory| memory.disabled_until.as_deref()) else {
        return false;
    };
    DateTime::parse_from_rfc3339(disabled_until)
        .map(|until| until.with_timezone(&Utc) > Utc::now())
        .unwrap_or(true)
}

/// 估算提取来源轮次 token 数。
///
/// 参数:
/// - `turns`: 来源轮次
///
/// 返回:
/// - token 估算值
fn estimate_turn_tokens(turns: &[Turn]) -> usize {
    let mut combined = String::new();
    for turn in turns {
        combined.push_str(&turn.user_content);
        combined.push_str(&turn.assistant_content);
    }
    crate::token_estimate::estimate_tokens(&combined)
}

/// 估算提取来源轮次字符数。
///
/// 参数:
/// - `turns`: 来源轮次
///
/// 返回:
/// - 字符数
pub(super) fn estimate_turn_chars(turns: &[Turn]) -> usize {
    turns
        .iter()
        .map(|turn| turn.user_content.chars().count() + turn.assistant_content.chars().count())
        .sum::<usize>()
}

/// 构造默认 Session Memory 摘要。
///
/// 参数:
/// - `turns`: 新增来源轮次
/// - `previous`: 既有会话工作记忆
///
/// 返回:
/// - 会话工作记忆摘要
fn default_summary(turns: &[Turn], previous: Option<&SessionMemory>) -> String {
    let mut lines = Vec::new();
    lines.push("当前会话工作记忆：".to_string());
    if let Some(memory) = previous.filter(|memory| !memory.summary.trim().is_empty()) {
        lines.push("既有摘要：".to_string());
        lines.push(clip_text(&memory.summary, 2_000));
    }
    lines.push("新增轮次：".to_string());
    for turn in turns.iter().rev().take(8).rev() {
        lines.push(format!(
            "- seq {} user: {}",
            turn.seq,
            clip_text(&turn.user_content, 400)
        ));
        lines.push(format!(
            "  assistant: {}",
            clip_text(&turn.assistant_content, 600)
        ));
    }
    clip_text(&lines.join("\n"), 6_000)
}

/// 按字符数裁剪文本。
///
/// 参数:
/// - `value`: 原始文本
/// - `max_chars`: 最大字符数
///
/// 返回:
/// - 裁剪后的文本
pub(super) fn clip_text(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    if max_chars <= 3 {
        return ".".repeat(max_chars);
    }
    let mut output = value
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    output.push_str("...");
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_db() -> (TempDir, ConversationDb) {
        let temp = tempfile::tempdir().unwrap();
        let db = ConversationDb::open(temp.path()).unwrap();
        (temp, db)
    }

    #[test]
    fn extractor_updates_boundary_after_completed_turns() {
        let (_temp, db) = test_db();
        db.start_turn("turn_1", "first task").unwrap();
        db.complete_turn("turn_1", "first answer", None).unwrap();
        db.start_turn("turn_2", "still running").unwrap();

        let outcome = extract_after_turn(
            &db,
            "default",
            ExtractionConfig::always(),
            |turns, previous| {
                assert!(previous.is_none());
                assert_eq!(turns.len(), 1);
                Ok("task and answer summary".to_string())
            },
        )
        .unwrap();

        let memory = load_memory(&db, "default").unwrap().unwrap();
        assert!(outcome.extracted);
        assert_eq!(memory.summary, "task and answer summary");
        assert_eq!(memory.last_summarized_turn_id.as_deref(), Some("turn_1"));
        assert_eq!(memory.last_summarized_seq, 1);
        assert_eq!(memory.source_turn_count, 1);
        assert_eq!(memory.token_estimate, 4);
        assert_eq!(memory.consecutive_failures, 0);
    }

    #[test]
    fn extractor_failure_preserves_previous_summary() {
        let (_temp, db) = test_db();
        db.start_turn("turn_1", "old task").unwrap();
        db.complete_turn("turn_1", "old answer", None).unwrap();
        upsert_memory(
            &db,
            NewSessionMemory {
                session_id: "default".to_string(),
                summary: "previous summary".to_string(),
                last_summarized_turn_id: Some("turn_1".to_string()),
                last_summarized_seq: 1,
                checkpoint_id: None,
                source_turn_count: 1,
                token_estimate: 10,
            },
        )
        .unwrap();
        db.start_turn("turn_2", "new task").unwrap();
        db.complete_turn("turn_2", "new answer", None).unwrap();

        let result = extract_after_turn(
            &db,
            "default",
            ExtractionConfig::always(),
            |_turns, _previous| anyhow::bail!("extractor failed"),
        );

        let memory = load_memory(&db, "default").unwrap().unwrap();
        assert!(result.is_err());
        assert_eq!(memory.summary, "previous summary");
        assert_eq!(memory.last_summarized_seq, 1);
        assert_eq!(memory.consecutive_failures, 1);
        assert_eq!(memory.last_error.as_deref(), Some("extractor failed"));
    }

    #[test]
    fn extractor_failure_writes_recovery_record() {
        let (_temp, db) = test_db();
        db.start_turn("turn_1", "new task").unwrap();
        db.complete_turn("turn_1", "new answer", None).unwrap();

        let result = extract_after_turn(
            &db,
            "default",
            ExtractionConfig::always(),
            |_turns, _previous| anyhow::bail!("extractor failed"),
        );

        let recovery = crate::state::failure_recovery::recovery_snapshot(&db, "default").unwrap();
        let latest = recovery.latest.expect("recovery record");
        assert!(result.is_err());
        assert_eq!(
            latest.kind,
            crate::state::failure_recovery::FailureKind::SessionMemoryExtractionFailed
        );
        assert_eq!(
            latest.status,
            crate::state::failure_recovery::RecoveryStatus::Observed
        );
        assert_eq!(latest.retry_count, 1);
        assert_eq!(latest.reason, "extractor failed");
    }

    #[test]
    fn session_memory_breaker_disables_after_three_failures() {
        let (_temp, db) = test_db();
        db.start_turn("turn_1", "new task").unwrap();
        db.complete_turn("turn_1", "new answer", None).unwrap();

        for _ in 0..3 {
            let _ = extract_after_turn(
                &db,
                "default",
                ExtractionConfig::always(),
                |_turns, _previous| anyhow::bail!("extractor failed"),
            );
        }

        let memory = load_memory(&db, "default").unwrap().unwrap();
        let outcome = extract_after_turn(
            &db,
            "default",
            ExtractionConfig::always(),
            |_turns, _previous| anyhow::bail!("summarizer should not be called"),
        )
        .unwrap();

        assert_eq!(memory.consecutive_failures, 3);
        assert!(memory.disabled_until.is_some());
        assert!(!outcome.extracted);
        assert_eq!(
            outcome.skipped_reason.as_deref(),
            Some("session memory extraction is disabled")
        );
    }
}
