use super::model::{
    NewToolCallRecord, NewToolOutputReplacement, NewToolResultRecord, ToolCallRecord,
    ToolCallStatus, ToolExchangeRecord, ToolHistorySummary, ToolOutputReplacement,
    ToolResultRecord,
};
use crate::state::turns::ConversationDb;
use crate::state::StateStore;
use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use sha2::{Digest, Sha256};

impl StateStore {
    /// 返回指定轮次已经持久化的工具调用数量。
    ///
    /// 参数:
    /// - `turn_id`: 轮次标识
    ///
    /// 返回:
    /// - 已记录工具调用数量
    pub(crate) fn tool_call_count_for_turn(&self, turn_id: &str) -> Result<usize> {
        let conn = self.conv_db.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM tool_calls WHERE session_id = ?1 AND turn_id = ?2",
            params![self.session_id, turn_id],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    /// 记录工具调用开始。
    ///
    /// 参数:
    /// - `turn_id`: 当前轮次标识
    /// - `seq`: 当前轮内工具调用顺序
    /// - `provider_call_id`: provider 工具调用标识
    /// - `tool_name`: 工具名称
    /// - `arguments`: 工具参数 JSON 文本
    ///
    /// 返回:
    /// - 写入是否成功
    pub(crate) fn record_tool_call_started(
        &self,
        turn_id: &str,
        seq: usize,
        provider_call_id: &str,
        tool_name: &str,
        arguments: &str,
    ) -> Result<()> {
        insert_tool_call(
            &self.conv_db,
            NewToolCallRecord {
                session_id: self.session_id.clone(),
                turn_id: turn_id.to_string(),
                seq,
                provider_call_id: provider_call_id.to_string(),
                tool_name: tool_name.to_string(),
                arguments: arguments.to_string(),
            },
        )
    }

    /// 记录携带 assistant 子轮上下文的工具调用开始事件。
    ///
    /// 参数:
    /// - `turn_id`: 当前轮次标识
    /// - `seq`: 当前轮内工具调用顺序
    /// - `assistant_round`: 产生该工具调用的模型子轮编号
    /// - `assistant_reasoning`: 该模型子轮返回的思考内容
    /// - `provider_call_id`: provider 工具调用标识
    /// - `tool_name`: 工具名称
    /// - `arguments`: 工具参数 JSON 文本
    ///
    /// 返回:
    /// - 写入是否成功
    pub(crate) fn record_tool_call_started_with_context(
        &self,
        turn_id: &str,
        seq: usize,
        assistant_round: usize,
        assistant_reasoning: Option<&str>,
        provider_call_id: &str,
        tool_name: &str,
        arguments: &str,
    ) -> Result<()> {
        insert_tool_call_with_context(
            &self.conv_db,
            NewToolCallRecord {
                session_id: self.session_id.clone(),
                turn_id: turn_id.to_string(),
                seq,
                provider_call_id: provider_call_id.to_string(),
                tool_name: tool_name.to_string(),
                arguments: arguments.to_string(),
            },
            assistant_round,
            assistant_reasoning,
        )
    }

    /// 记录统一工具网关解包后的展示信息。
    ///
    /// 参数:
    /// - `provider_call_id`: provider 工具调用标识
    /// - `tool_name`: 真实工具名称
    /// - `arguments`: 真实工具参数 JSON 文本
    ///
    /// 返回:
    /// - 更新是否成功
    pub(crate) fn record_tool_call_display(
        &self,
        provider_call_id: &str,
        tool_name: &str,
        arguments: &str,
    ) -> Result<()> {
        update_tool_call_display(
            &self.conv_db,
            &self.session_id,
            provider_call_id,
            tool_name,
            arguments,
        )
    }

    /// 记录工具调用结果。
    ///
    /// 参数:
    /// - `turn_id`: 当前轮次标识
    /// - `provider_call_id`: provider 工具调用标识
    /// - `ok`: 工具是否成功
    /// - `result_preview`: 模型可见工具结果
    /// - `result_ref`: 可选完整结果引用
    /// - `error`: 可选错误信息
    /// - `original_chars`: 原始输出字符数
    ///
    /// 返回:
    /// - 写入是否成功
    pub(crate) fn record_tool_result_completed(
        &self,
        turn_id: &str,
        provider_call_id: &str,
        ok: bool,
        result_preview: &str,
        result_ref: Option<&str>,
        error: Option<&str>,
        original_chars: usize,
    ) -> Result<()> {
        insert_tool_result(
            &self.conv_db,
            NewToolResultRecord {
                session_id: self.session_id.clone(),
                turn_id: turn_id.to_string(),
                provider_call_id: provider_call_id.to_string(),
                ok,
                result_preview: result_preview.to_string(),
                result_ref: result_ref.map(str::to_string),
                error: error.map(str::to_string),
                original_chars,
            },
        )
    }

    /// 读取当前会话工具历史摘要。
    ///
    /// 返回:
    /// - 工具历史摘要
    pub(crate) fn tool_history_summary(&self) -> Result<ToolHistorySummary> {
        summarize_tool_history(&self.conv_db, &self.session_id)
    }

    /// 结算指定轮次中的未完成工具调用。
    ///
    /// 参数:
    /// - `turn_ids`: 需要结算的轮次标识列表
    ///
    /// 返回:
    /// - 被标记为中断的工具调用数量
    pub(crate) fn settle_pending_tool_calls_for_turns(&self, turn_ids: &[String]) -> Result<usize> {
        settle_pending_tool_calls_for_turns(&self.conv_db, &self.session_id, turn_ids)
    }

    /// 保存被裁剪工具输出并记录稳定 replacement。
    ///
    /// 参数:
    /// - `provider_call_id`: provider 工具调用标识
    /// - `raw_output`: 原始工具输出
    /// - `context_output`: 模型可见工具输出
    ///
    /// 返回:
    /// - 完整输出引用
    pub(crate) fn save_clipped_tool_output_replacement(
        &self,
        provider_call_id: &str,
        raw_output: &str,
        context_output: &str,
    ) -> Result<Option<String>> {
        if raw_output == context_output {
            return Ok(None);
        }
        let result_ref = self.write_tool_result_archive(provider_call_id, raw_output)?;
        upsert_tool_output_replacement(
            &self.conv_db,
            NewToolOutputReplacement {
                provider_call_id: provider_call_id.to_string(),
                session_id: self.session_id.clone(),
                replacement: context_output.to_string(),
                original_chars: raw_output.chars().count(),
                result_ref: result_ref.clone(),
                policy: "context_clip".to_string(),
            },
        )?;
        Ok(Some(result_ref))
    }

    /// 把完整工具输出写入会话归档目录。
    ///
    /// 参数:
    /// - `provider_call_id`: provider 工具调用标识
    /// - `raw_output`: 待归档的完整输出
    ///
    /// 返回:
    /// - 相对于会话目录的归档引用
    pub(crate) fn write_tool_result_archive(
        &self,
        provider_call_id: &str,
        raw_output: &str,
    ) -> Result<String> {
        let output_dir = self.state_dir.join("tool-results");
        std::fs::create_dir_all(&output_dir)?;
        let file_name = format!(
            "{}_{}.txt",
            sanitize_reference_id(provider_call_id),
            short_reference_hash(provider_call_id)
        );
        let output_path = output_dir.join(&file_name);
        std::fs::write(&output_path, raw_output)?;
        Ok(format!("tool-results/{file_name}"))
    }
}

/// 插入或更新工具调用记录。
///
/// 参数:
/// - `db`: 对话数据库
/// - `record`: 待写入工具调用
///
/// 返回:
/// - 写入是否成功
pub(in crate::state) fn insert_tool_call(
    db: &ConversationDb,
    record: NewToolCallRecord,
) -> Result<()> {
    insert_tool_call_with_context(db, record, 0, None)
}

/// 插入或更新携带 assistant 子轮上下文的工具调用记录。
///
/// 参数:
/// - `db`: 对话数据库
/// - `record`: 待写入工具调用
/// - `assistant_round`: 产生工具调用的模型子轮编号
/// - `assistant_reasoning`: 该子轮返回的思考内容
///
/// 返回:
/// - 写入是否成功
pub(in crate::state) fn insert_tool_call_with_context(
    db: &ConversationDb,
    record: NewToolCallRecord,
    assistant_round: usize,
    assistant_reasoning: Option<&str>,
) -> Result<()> {
    let conn = db.conn.lock().unwrap();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO tool_calls (
            id, session_id, turn_id, seq, assistant_round, assistant_reasoning,
            provider_call_id, tool_name, arguments, status, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)
         ON CONFLICT(session_id, provider_call_id) DO UPDATE SET
            turn_id = excluded.turn_id,
            seq = excluded.seq,
            assistant_round = excluded.assistant_round,
            assistant_reasoning = excluded.assistant_reasoning,
            tool_name = excluded.tool_name,
            arguments = excluded.arguments,
            updated_at = excluded.updated_at",
        params![
            format!(
                "tool_call_{}_{}",
                Utc::now().timestamp_millis(),
                rand::random::<u16>()
            ),
            record.session_id,
            record.turn_id,
            record.seq as i64,
            assistant_round as i64,
            assistant_reasoning,
            record.provider_call_id,
            record.tool_name,
            record.arguments,
            ToolCallStatus::Pending.as_str(),
            now,
        ],
    )?;
    Ok(())
}

/// 更新工具调用的界面展示信息。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `provider_call_id`: provider 工具调用标识
/// - `tool_name`: 真实工具名称
/// - `arguments`: 真实工具参数 JSON 文本
///
/// 返回:
/// - 更新是否成功
fn update_tool_call_display(
    db: &ConversationDb,
    session_id: &str,
    provider_call_id: &str,
    tool_name: &str,
    arguments: &str,
) -> Result<()> {
    let conn = db.conn.lock().unwrap();
    conn.execute(
        "UPDATE tool_calls
         SET display_tool_name = ?1, display_arguments = ?2, updated_at = ?3
         WHERE session_id = ?4 AND provider_call_id = ?5",
        params![
            tool_name,
            arguments,
            Utc::now().to_rfc3339(),
            session_id,
            provider_call_id,
        ],
    )?;
    Ok(())
}

/// 插入工具结果并更新调用状态。
///
/// 参数:
/// - `db`: 对话数据库
/// - `record`: 待写入工具结果
///
/// 返回:
/// - 写入是否成功
pub(in crate::state) fn insert_tool_result(
    db: &ConversationDb,
    record: NewToolResultRecord,
) -> Result<()> {
    let mut conn = db.conn.lock().unwrap();
    let now = Utc::now().to_rfc3339();
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO tool_results (
            id, session_id, turn_id, provider_call_id, ok, result_preview,
            result_ref, error, original_chars, created_at, completed_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)
         ON CONFLICT(session_id, provider_call_id) DO UPDATE SET
            turn_id = excluded.turn_id,
            ok = excluded.ok,
            result_preview = excluded.result_preview,
            result_ref = excluded.result_ref,
            error = excluded.error,
            original_chars = excluded.original_chars,
            completed_at = excluded.completed_at",
        params![
            format!(
                "tool_result_{}_{}",
                Utc::now().timestamp_millis(),
                rand::random::<u16>()
            ),
            &record.session_id,
            &record.turn_id,
            &record.provider_call_id,
            if record.ok { 1_i64 } else { 0_i64 },
            &record.result_preview,
            &record.result_ref,
            &record.error,
            record.original_chars as i64,
            now,
        ],
    )?;
    tx.execute(
        "UPDATE tool_calls
         SET status = ?1, updated_at = ?2
         WHERE session_id = ?3 AND provider_call_id = ?4",
        params![
            if record.ok {
                ToolCallStatus::Completed.as_str()
            } else {
                ToolCallStatus::Error.as_str()
            },
            now,
            &record.session_id,
            &record.provider_call_id,
        ],
    )?;
    tx.commit()?;
    Ok(())
}

/// 写入或更新工具输出替换记录。
///
/// 参数:
/// - `db`: 对话数据库
/// - `record`: 待写入工具输出替换
///
/// 返回:
/// - 写入是否成功
pub(in crate::state) fn upsert_tool_output_replacement(
    db: &ConversationDb,
    record: NewToolOutputReplacement,
) -> Result<()> {
    let conn = db.conn.lock().unwrap();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO tool_output_replacements (
            provider_call_id, session_id, replacement, original_chars,
            result_ref, policy, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(provider_call_id) DO UPDATE SET
            session_id = excluded.session_id,
            replacement = excluded.replacement,
            original_chars = excluded.original_chars,
            result_ref = excluded.result_ref,
            policy = excluded.policy",
        params![
            record.provider_call_id,
            record.session_id,
            record.replacement,
            record.original_chars as i64,
            record.result_ref,
            record.policy,
            now,
        ],
    )?;
    Ok(())
}

/// 汇总当前会话工具历史。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
///
/// 返回:
/// - 工具历史摘要
pub(in crate::state) fn summarize_tool_history(
    db: &ConversationDb,
    session_id: &str,
) -> Result<ToolHistorySummary> {
    let conn = db.conn.lock().unwrap();
    let (call_count, pending_count, error_count): (i64, i64, i64) = conn.query_row(
        "SELECT
            COUNT(*),
            COALESCE(SUM(CASE WHEN status = 'pending' THEN 1 ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN status = 'error' OR status = 'interrupted' THEN 1 ELSE 0 END), 0)
         FROM tool_calls WHERE session_id = ?1",
        params![session_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    let result_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tool_results WHERE session_id = ?1",
        params![session_id],
        |row| row.get(0),
    )?;
    let replacement_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tool_output_replacements WHERE session_id = ?1",
        params![session_id],
        |row| row.get(0),
    )?;
    let latest = conn
        .query_row(
            "SELECT COALESCE(display_tool_name, tool_name), status FROM tool_calls
             WHERE session_id = ?1
             ORDER BY updated_at DESC, seq DESC
             LIMIT 1",
            params![session_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    Ok(ToolHistorySummary {
        call_count: call_count as usize,
        result_count: result_count as usize,
        pending_count: pending_count as usize,
        error_count: error_count as usize,
        replacement_count: replacement_count as usize,
        latest_tool_name: latest.as_ref().map(|(name, _)| name.clone()),
        latest_status: latest.map(|(_, status)| ToolCallStatus::from_str(&status)),
    })
}

/// 读取指定轮次的工具调用交换记录。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `turn_id`: 轮次标识
///
/// 返回:
/// - 按轮内顺序排列的工具调用交换记录
pub(in crate::state) fn load_tool_exchanges_for_turn(
    db: &ConversationDb,
    session_id: &str,
    turn_id: &str,
) -> Result<Vec<ToolExchangeRecord>> {
    let conn = db.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT
            c.id, c.session_id, c.turn_id, c.seq, c.assistant_round,
            c.assistant_reasoning, c.provider_call_id, c.tool_name, c.arguments,
            c.display_tool_name, c.display_arguments, c.status, c.created_at, c.updated_at,
            r.id, r.session_id, r.turn_id, r.provider_call_id, r.ok,
            r.result_preview, r.result_ref, r.error, r.original_chars,
            r.created_at, r.completed_at,
            p.provider_call_id, p.session_id, p.replacement, p.original_chars,
            p.result_ref, p.policy, p.created_at
         FROM tool_calls c
         LEFT JOIN tool_results r
            ON r.session_id = c.session_id
           AND r.provider_call_id = c.provider_call_id
         LEFT JOIN tool_output_replacements p
            ON p.session_id = c.session_id
           AND p.provider_call_id = c.provider_call_id
         WHERE c.session_id = ?1 AND c.turn_id = ?2
         ORDER BY c.seq ASC, c.created_at ASC",
    )?;
    let rows = stmt.query_map(params![session_id, turn_id], |row| {
        let call = ToolCallRecord {
            id: row.get(0)?,
            session_id: row.get(1)?,
            turn_id: row.get(2)?,
            seq: row.get::<_, i64>(3)? as usize,
            assistant_round: row.get::<_, i64>(4)? as usize,
            assistant_reasoning: row.get(5)?,
            provider_call_id: row.get(6)?,
            tool_name: row.get(7)?,
            arguments: row.get(8)?,
            display_tool_name: row.get(9)?,
            display_arguments: row.get(10)?,
            status: ToolCallStatus::from_str(&row.get::<_, String>(11)?),
            created_at: row.get(12)?,
            updated_at: row.get(13)?,
        };
        let result_id: Option<String> = row.get(14)?;
        let result = match result_id {
            Some(id) => Some(ToolResultRecord {
                id,
                session_id: row.get(15)?,
                turn_id: row.get(16)?,
                provider_call_id: row.get(17)?,
                ok: row.get::<_, i64>(18)? == 1,
                result_preview: row.get(19)?,
                result_ref: row.get(20)?,
                error: row.get(21)?,
                original_chars: row.get::<_, i64>(22)? as usize,
                created_at: row.get(23)?,
                completed_at: row.get(24)?,
            }),
            None => None,
        };
        let replacement_id: Option<String> = row.get(25)?;
        let replacement = match replacement_id {
            Some(provider_call_id) => Some(ToolOutputReplacement {
                provider_call_id,
                session_id: row.get(26)?,
                replacement: row.get(27)?,
                original_chars: row.get::<_, i64>(28)? as usize,
                result_ref: row.get(29)?,
                policy: row.get(30)?,
                created_at: row.get(31)?,
            }),
            None => None,
        };
        Ok(ToolExchangeRecord {
            call,
            result,
            replacement,
        })
    })?;
    Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
}

/// 将指定轮次中的 pending 工具调用标记为 interrupted。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `turn_ids`: 轮次标识列表
///
/// 返回:
/// - 被更新的工具调用数量
pub(in crate::state) fn settle_pending_tool_calls_for_turns(
    db: &ConversationDb,
    session_id: &str,
    turn_ids: &[String],
) -> Result<usize> {
    if turn_ids.is_empty() {
        return Ok(0);
    }
    let conn = db.conn.lock().unwrap();
    let now = Utc::now().to_rfc3339();
    let mut updated = 0usize;
    for turn_id in turn_ids {
        // 1. 找出仍 pending 的工具，便于补写可展示的中断结果
        let mut pending_stmt = conn.prepare(
            "SELECT provider_call_id
             FROM tool_calls
             WHERE session_id = ?1 AND turn_id = ?2 AND status = 'pending'",
        )?;
        let pending_ids = pending_stmt
            .query_map(params![session_id, turn_id], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(pending_stmt);

        // 2. 将 pending 工具标记为 interrupted
        updated += conn.execute(
            "UPDATE tool_calls
             SET status = ?1, updated_at = ?2
             WHERE session_id = ?3 AND turn_id = ?4 AND status = 'pending'",
            params![
                ToolCallStatus::Interrupted.as_str(),
                now,
                session_id,
                turn_id,
            ],
        )?;

        // 3. 为每个中断工具写入结果记录，供时间线展示详情
        let message = "tool call was interrupted before a result was recorded";
        for provider_call_id in pending_ids {
            conn.execute(
                "INSERT OR IGNORE INTO tool_results (
                    id, session_id, turn_id, provider_call_id, ok, result_preview,
                    result_ref, error, original_chars, created_at, completed_at
                 ) VALUES (
                    lower(hex(randomblob(16))),
                    ?1,
                    ?2,
                    ?3,
                    0,
                    ?4,
                    NULL,
                    ?4,
                    ?5,
                    ?6,
                    ?6
                 )",
                params![
                    session_id,
                    turn_id,
                    provider_call_id,
                    message,
                    message.chars().count() as i64,
                    now,
                ],
            )?;
        }
    }
    Ok(updated)
}

/// 清理 provider 调用标识为安全文件名。
///
/// 参数:
/// - `value`: provider 调用标识
///
/// 返回:
/// - 安全文件名片段
fn sanitize_reference_id(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "tool_call".to_string()
    } else {
        sanitized
    }
}

/// 生成 provider 调用标识的稳定短哈希。
///
/// 参数:
/// - `value`: provider 调用标识
///
/// 返回:
/// - 十六进制短哈希
fn short_reference_hash(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    hex::encode(digest)[..12].to_string()
}

#[cfg(test)]
#[path = "repository_tests.rs"]
mod tests;
