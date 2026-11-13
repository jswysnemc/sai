use crate::state::tool_history::load_tool_exchanges_for_turn;
use crate::state::turns::{ConversationDb, Turn};
use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};

/// 会话记忆两次模型提取之间必须新增的 token 数，恰好达到时仍不触发。
pub(super) const EXTRACTION_TOKEN_INTERVAL: usize = 40_000;
const EXTRACTION_LEASE_SECONDS: i64 = 600;

/// 【会话记忆】【间隔边界】最后一轮及其已累计正文、工具 token 数。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TokenBoundary {
    pub seq: i64,
    pub turn_tokens: usize,
}

/// 【会话记忆】【提取租约】限制同时请求，并防止压缩后提交旧结果。
#[derive(Debug, Clone)]
pub(crate) struct ExtractionTicket {
    pub id: String,
    pub boundary: TokenBoundary,
}

/// 【会话记忆】【间隔统计】统计单轮新增正文和工具交换，不累计重放历史或私有推理。
/// 参数：数据库、会话标识和轮次；返回：本轮可见文本 token 数。
pub(super) fn turn_tokens(db: &ConversationDb, session_id: &str, turn: &Turn) -> Result<usize> {
    let mut tokens = crate::token_estimate::estimate_tokens(&turn.user_content).saturating_add(
        crate::token_estimate::estimate_tokens(&turn.assistant_content),
    );
    let exchanges = load_tool_exchanges_for_turn(db, session_id, &turn.turn_id)?;
    if exchanges.is_empty() {
        for report in &turn.tool_reports {
            tokens = tokens.saturating_add(crate::token_estimate::estimate_tokens(report));
        }
    }
    for exchange in exchanges {
        tokens = tokens
            .saturating_add(crate::token_estimate::estimate_tokens(
                &exchange.call.tool_name,
            ))
            .saturating_add(crate::token_estimate::estimate_tokens(
                &exchange.call.arguments,
            ));
        if let Some(result) = exchange.result {
            tokens = tokens.saturating_add(crate::token_estimate::estimate_tokens(
                &result.result_preview,
            ));
        }
    }
    Ok(tokens)
}

/// 【会话记忆】【边界快照】记录末轮当前 token 数，支持同一轮内压缩后继续累计。
/// 参数：数据库、会话标识和活动分支轮次；返回：间隔边界。
pub(crate) fn capture_boundary(
    db: &ConversationDb,
    session_id: &str,
    turns: &[Turn],
) -> Result<TokenBoundary> {
    turns.last().map_or_else(
        || Ok(TokenBoundary::default()),
        |turn| {
            Ok(TokenBoundary {
                seq: turn.seq,
                turn_tokens: turn_tokens(db, session_id, turn)?,
            })
        },
    )
}

/// 【会话记忆】【读取边界】读取持久化的累计起点。
/// 参数：数据库连接和会话标识；返回：边界，未初始化时返回空。
fn load_boundary(conn: &Connection, session_id: &str) -> Result<Option<TokenBoundary>> {
    Ok(conn.query_row(
        "SELECT baseline_seq, baseline_turn_tokens FROM session_memory_extraction_interval WHERE session_id = ?1",
        [session_id],
        |row| Ok(TokenBoundary { seq: row.get(0)?, turn_tokens: row.get::<_, i64>(1)? as usize }),
    ).optional()?)
}

/// 【会话记忆】【申请提取】新增 token 严格超过间隔后原子申请租约。
/// 参数：数据库、会话标识、已结束的活动轮次、旧记忆边界和间隔；返回：租约或跳过。
pub(super) fn claim_extraction(
    db: &ConversationDb,
    session_id: &str,
    turns: &[Turn],
    previous_seq: i64,
    min_new_tokens: usize,
) -> Result<Option<ExtractionTicket>> {
    // 1. 【会话记忆】【兼容旧数据】已有记忆从已覆盖轮次继续累计
    let initial = TokenBoundary {
        seq: previous_seq,
        turn_tokens: turns
            .iter()
            .find(|turn| turn.seq == previous_seq)
            .map(|turn| turn_tokens(db, session_id, turn))
            .transpose()?
            .unwrap_or_default(),
    };
    let baseline = db.with_conn(|conn| {
        conn.execute(
            "INSERT OR IGNORE INTO session_memory_extraction_interval (session_id, baseline_seq, baseline_turn_tokens) VALUES (?1, ?2, ?3)",
            params![session_id, initial.seq, initial.turn_tokens as i64],
        )?;
        Ok(load_boundary(conn, session_id)?.unwrap_or_default())
    })?;
    // 2. 【会话记忆】【新增用量】仅统计边界后新增的文本，末轮只扣一次快照
    let mut new_tokens = 0usize;
    for turn in turns.iter().filter(|turn| turn.seq >= baseline.seq) {
        let tokens = turn_tokens(db, session_id, turn)?;
        new_tokens = new_tokens.saturating_add(if turn.seq == baseline.seq {
            tokens.saturating_sub(baseline.turn_tokens)
        } else {
            tokens
        });
    }
    if new_tokens <= min_new_tokens {
        return Ok(None);
    }
    let ticket = ExtractionTicket {
        id: uuid::Uuid::new_v4().to_string(),
        boundary: capture_boundary(db, session_id, turns)?,
    };
    let now = Utc::now().timestamp();
    // 3. 【会话记忆】【并发保护】比较边界并占用租约，崩溃后过期租约可以重试
    let claimed = db.with_conn(|conn| {
        Ok(conn.execute(
            "UPDATE session_memory_extraction_interval SET pending_id = ?1, pending_until = ?2
         WHERE session_id = ?3 AND baseline_seq = ?4 AND baseline_turn_tokens = ?5
         AND (pending_id IS NULL OR pending_until <= ?6)",
            params![
                ticket.id,
                now + EXTRACTION_LEASE_SECONDS,
                session_id,
                baseline.seq,
                baseline.turn_tokens as i64,
                now
            ],
        )? > 0)
    })?;
    Ok(claimed.then_some(ticket))
}

/// 【会话记忆】【租约校验】检查结果是否仍属于本次提取。
/// 参数：连接、会话标识和租约；返回：租约是否匹配。
pub(super) fn owns_ticket(
    conn: &Connection,
    session_id: &str,
    ticket: &ExtractionTicket,
) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM session_memory_extraction_interval WHERE session_id = ?1 AND pending_id = ?2)",
        params![session_id, ticket.id], |row| row.get(0),
    )?)
}

/// 【会话记忆】【提交边界】成功提取或压缩后重置间隔，同时使旧租约失效。
/// 参数：连接、会话标识和新的边界；返回：写入结果。
pub(crate) fn reset_boundary(
    conn: &Connection,
    session_id: &str,
    boundary: &TokenBoundary,
) -> Result<()> {
    conn.execute(
        "INSERT INTO session_memory_extraction_interval (session_id, baseline_seq, baseline_turn_tokens) VALUES (?1, ?2, ?3)
         ON CONFLICT(session_id) DO UPDATE SET baseline_seq = excluded.baseline_seq,
         baseline_turn_tokens = excluded.baseline_turn_tokens, pending_id = NULL, pending_until = NULL",
        params![session_id, boundary.seq, boundary.turn_tokens as i64],
    )?;
    Ok(())
}

/// 【会话记忆】【失败释放】失败不推进边界，且不得清除其它任务租约。
/// 参数：数据库、会话标识和租约；返回：是否释放了当前租约。
pub(super) fn release_ticket(
    db: &ConversationDb,
    session_id: &str,
    ticket: &ExtractionTicket,
) -> Result<bool> {
    db.with_conn(|conn| Ok(conn.execute(
        "UPDATE session_memory_extraction_interval SET pending_id = NULL, pending_until = NULL WHERE session_id = ?1 AND pending_id = ?2",
        params![session_id, ticket.id],
    )? > 0))
}

#[cfg(test)]
#[path = "extraction_interval_tests.rs"]
mod tests;
