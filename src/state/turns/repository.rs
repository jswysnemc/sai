use super::model::{Turn, TurnStatus};
use super::schema::open_connection;
use anyhow::{bail, Result};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::path::Path;
use std::sync::Mutex;

pub struct ConversationDb {
    pub(in crate::state) conn: Mutex<Connection>,
}

#[derive(Debug, Clone, Copy, Default)]
pub(in crate::state) struct SessionSummaryTurnStats {
    pub tail_turn_count: usize,
    pub context_chars: usize,
}

impl std::fmt::Debug for ConversationDb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConversationDb").finish_non_exhaustive()
    }
}

impl ConversationDb {
    /// 打开对话数据库。
    ///
    /// 参数:
    /// - `state_dir`: 状态目录
    ///
    /// 返回:
    /// - 对话数据库仓储
    pub fn open(state_dir: &Path) -> Result<Self> {
        Ok(Self {
            conn: Mutex::new(open_connection(state_dir)?),
        })
    }

    /// 使用数据库连接执行只暴露连接边界的操作。
    ///
    /// 参数:
    /// - `operation`: 需要在连接上执行的操作
    ///
    /// 返回:
    /// - 操作结果
    pub(crate) fn with_conn<T>(
        &self,
        operation: impl FnOnce(&Connection) -> Result<T>,
    ) -> Result<T> {
        let conn = self.conn.lock().unwrap();
        operation(&conn)
    }

    /// 开始一轮新对话。
    ///
    /// 参数:
    /// - `turn_id`: 当前轮唯一标识
    /// - `content`: 已生成的部分助手正文
    /// - `reasoning`: 可选的部分推理内容
    /// - `user_content`: 用户输入
    ///
    /// 返回:
    /// - 写入是否成功
    #[allow(dead_code)]
    pub fn start_turn(&self, turn_id: &str, user_content: &str) -> Result<()> {
        self.start_turn_with_images(turn_id, user_content, &[])
    }

    /// 开始一轮新对话并保存用户图片。
    ///
    /// 参数:
    /// - `turn_id`: 当前轮唯一标识
    /// - `user_content`: 用户输入
    /// - `user_image_urls`: 用户附件图片 data URL 列表
    ///
    /// 返回:
    /// - 写入是否成功
    pub fn start_turn_with_images(
        &self,
        turn_id: &str,
        user_content: &str,
        user_image_urls: &[String],
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        self.insert_turn_locked(
            &conn,
            InsertTurn {
                turn_id,
                user_content,
                user_image_urls,
                user_timestamp: &now,
                assistant_content: "",
                assistant_reasoning: None,
                assistant_timestamp: None,
                status: TurnStatus::Running,
                tool_reports: &[],
            },
        )
    }

    /// 完成指定对话轮次。
    ///
    /// 参数:
    /// - `turn_id`: 当前轮唯一标识
    /// - `content`: 助手回复
    /// - `reasoning`: 可选推理内容
    ///
    /// 返回:
    /// - 更新是否成功
    pub fn complete_turn(
        &self,
        turn_id: &str,
        content: &str,
        reasoning: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE turns
             SET assistant_content = ?1,
                 assistant_reasoning = ?2,
                 assistant_timestamp = ?3,
                 status = 'completed'
             WHERE turn_id = ?4",
            params![content, reasoning, now, turn_id],
        )?;
        Ok(())
    }

    /// 标记指定轮次已中断。
    ///
    /// 参数:
    /// - `turn_id`: 当前轮唯一标识
    ///
    /// 返回:
    /// - 更新是否成功
    pub fn interrupt_turn(
        &self,
        turn_id: &str,
        content: &str,
        reasoning: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE turns
             SET assistant_content = ?1,
                 assistant_reasoning = ?2,
                 assistant_timestamp = ?3,
                 status = 'interrupted'
             WHERE turn_id = ?4 AND status = 'running'",
            params![content, reasoning, now, turn_id],
        )?;
        Ok(())
    }

    /// 附加当前轮工具报告上下文。
    ///
    /// 参数:
    /// - `turn_id`: 当前轮唯一标识
    /// - `report`: 工具报告文本
    ///
    /// 返回:
    /// - 写入是否成功
    pub fn append_tool_report(&self, turn_id: &str, report: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let existing: Option<String> = conn
            .query_row(
                "SELECT tool_reports FROM turns WHERE turn_id = ?1",
                params![turn_id],
                |row| row.get(0),
            )
            .optional()?;
        let mut reports: Vec<String> = existing
            .as_deref()
            .and_then(|value| serde_json::from_str(value).ok())
            .unwrap_or_default();
        reports.push(report.to_string());
        conn.execute(
            "UPDATE turns SET tool_reports = ?1 WHERE turn_id = ?2",
            params![serde_json::to_string(&reports)?, turn_id],
        )?;
        Ok(())
    }

    /// 读取全部轮次。
    ///
    /// 返回:
    /// - 按序排列的轮次列表
    pub fn load_turns(&self) -> Result<Vec<Turn>> {
        let conn = self.conn.lock().unwrap();
        load_turns_with_sql(
            &conn,
            "SELECT turn_id, seq, user_content, user_image_urls, user_timestamp, assistant_content,
                    assistant_reasoning, assistant_timestamp, status, tool_reports
             FROM turns ORDER BY seq ASC",
            [],
        )
    }

    /// 读取指定 seq 之后的轮次。
    ///
    /// 参数:
    /// - `after_seq`: 起始 seq，不包含该 seq
    /// - `exclude_turn_id`: 可选排除轮次
    ///
    /// 返回:
    /// - tail turns
    pub(in crate::state) fn load_turns_after_seq(
        &self,
        after_seq: i64,
        exclude_turn_id: Option<&str>,
    ) -> Result<Vec<Turn>> {
        let conn = self.conn.lock().unwrap();
        match exclude_turn_id {
            Some(turn_id) => {
                let mut stmt = conn.prepare(
                    "SELECT turn_id, seq, user_content, user_image_urls, user_timestamp, assistant_content,
                            assistant_reasoning, assistant_timestamp, status, tool_reports
                     FROM turns WHERE seq > ?1 AND turn_id != ?2 ORDER BY seq ASC",
                )?;
                let turns = stmt
                    .query_map(params![after_seq, turn_id], map_turn)?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                Ok(turns)
            }
            None => load_turns_with_sql(
                &conn,
                "SELECT turn_id, seq, user_content, user_image_urls, user_timestamp, assistant_content,
                        assistant_reasoning, assistant_timestamp, status, tool_reports
                 FROM turns WHERE seq > ?1 ORDER BY seq ASC",
                params![after_seq],
            ),
        }
    }

    /// 聚合读取会话摘要需要的轮次统计。
    ///
    /// 参数:
    /// - `after_seq`: checkpoint 覆盖的最后序号
    ///
    /// 返回:
    /// - 当前轮次数、tail 轮次数和上下文字符估算
    pub(in crate::state) fn session_summary_turn_stats(
        &self,
        after_seq: i64,
    ) -> Result<SessionSummaryTurnStats> {
        let conn = self.conn.lock().unwrap();
        let context_chars: i64 = conn.query_row(
            "SELECT COALESCE(SUM(
                        CASE WHEN status != 'running' THEN
                            length(user_content)
                            + length(assistant_content)
                            + COALESCE(length(assistant_reasoning), 0)
                            + CASE WHEN tool_reports = '[]' THEN 0 ELSE length(tool_reports) END
                        ELSE 0 END
                    ), 0)
             FROM turns",
            [],
            |row| row.get(0),
        )?;
        let tail_turn_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM turns WHERE seq > ?1",
            params![after_seq],
            |row| row.get(0),
        )?;
        Ok(SessionSummaryTurnStats {
            tail_turn_count: tail_turn_count as usize,
            context_chars: context_chars as usize,
        })
    }

    /// 清空对话轮次。
    ///
    /// 返回:
    /// - 清空是否成功
    pub fn reset(&self) -> Result<()> {
        self.conn.lock().unwrap().execute_batch(
            "DELETE FROM turns;
             DELETE FROM compaction_checkpoints;
             DELETE FROM context_epoch_events;
             DELETE FROM context_epochs;
             DELETE FROM failure_recovery_records;
             DELETE FROM session_memory;
             DELETE FROM tool_calls;
             DELETE FROM tool_results;
             DELETE FROM tool_output_replacements;",
        )?;
        Ok(())
    }

    /// 撤销最后一轮对话。
    ///
    /// 返回:
    /// - 删除轮次数量和被撤销的用户输入
    pub fn undo_last_turn(&self) -> Result<(usize, Option<String>)> {
        let conn = self.conn.lock().unwrap();
        let last: Option<(String, String)> = conn
            .query_row(
                "SELECT turn_id, user_content FROM turns ORDER BY seq DESC LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        match last {
            Some((turn_id, user_content)) => {
                delete_turn_locked(&conn, &turn_id)?;
                Ok((1, Some(user_content)))
            }
            None => Ok((0, None)),
        }
    }

    /// 删除与预期标识匹配的最后一轮对话。
    ///
    /// 参数:
    /// - `expected_turn_id`: 前端准备重试的最后一轮标识
    ///
    /// 返回:
    /// - 删除轮次数量和被删除的用户输入
    pub fn rollback_last_turn(&self, expected_turn_id: &str) -> Result<(usize, Option<String>)> {
        let conn = self.conn.lock().unwrap();
        let last: Option<(String, String)> = conn
            .query_row(
                "SELECT turn_id, user_content FROM turns ORDER BY seq DESC LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((turn_id, user_content)) = last else {
            return Ok((0, None));
        };
        if turn_id != expected_turn_id {
            bail!("latest turn changed before retry: expected {expected_turn_id}, found {turn_id}");
        }
        delete_turn_locked(&conn, &turn_id)?;
        Ok((1, Some(user_content)))
    }

    /// 读取最后一轮标识和用户输入但不修改数据。
    ///
    /// 返回:
    /// - 最后一轮标识和用户输入
    pub fn last_turn_identity(&self) -> Result<Option<(String, String)>> {
        Ok(self
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT turn_id,user_content FROM turns ORDER BY seq DESC LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?)
    }

    /// 恢复所有陈旧运行中轮次为中断状态。
    ///
    /// 返回:
    /// - 被恢复的轮次标识列表
    pub fn recover_stale_running_turns(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();
        let stale_turns = {
            let mut stmt = conn.prepare(
                "SELECT turn_id,assistant_content,assistant_reasoning FROM turns WHERE status = 'running'",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })?;
            rows.collect::<std::result::Result<Vec<_>, _>>()?
        };
        if stale_turns.is_empty() {
            return Ok(Vec::new());
        }
        let mut recovered = Vec::new();
        for (turn_id, _content, _reasoning) in stale_turns {
            recovered.push(turn_id.clone());
            conn.execute(
                "UPDATE turns SET assistant_timestamp=?1,status='interrupted' WHERE turn_id=?2",
                params![now, turn_id],
            )?;
        }
        Ok(recovered)
    }

    /// 是否存在运行中轮次。
    ///
    /// 返回:
    /// - 是否存在运行中轮次
    #[allow(dead_code)]
    pub fn has_running_turns(&self) -> Result<bool> {
        let count: i64 = self.conn.lock().unwrap().query_row(
            "SELECT COUNT(*) FROM turns WHERE status = 'running'",
            [],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub(super) fn next_seq_locked(&self, conn: &Connection) -> Result<i64> {
        let max_seq: i64 =
            conn.query_row("SELECT COALESCE(MAX(seq), 0) FROM turns", [], |row| {
                row.get(0)
            })?;
        Ok(max_seq + 1)
    }

    pub(super) fn insert_turn_locked(&self, conn: &Connection, turn: InsertTurn<'_>) -> Result<()> {
        conn.execute(
            "INSERT INTO turns (
                turn_id, seq, user_content, user_image_urls, user_timestamp, assistant_content,
                assistant_reasoning, assistant_timestamp, status, tool_reports
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                turn.turn_id,
                self.next_seq_locked(conn)?,
                turn.user_content,
                serde_json::to_string(turn.user_image_urls)?,
                turn.user_timestamp,
                turn.assistant_content,
                turn.assistant_reasoning,
                turn.assistant_timestamp,
                turn.status.as_str(),
                serde_json::to_string(turn.tool_reports)?,
            ],
        )?;
        Ok(())
    }
}

/// 删除指定轮次和关联工具历史。
fn delete_turn_locked(conn: &Connection, turn_id: &str) -> Result<usize> {
    conn.execute(
        "DELETE FROM tool_output_replacements
         WHERE provider_call_id IN (
             SELECT provider_call_id FROM tool_calls WHERE turn_id = ?1
         )",
        params![turn_id],
    )?;
    conn.execute(
        "DELETE FROM tool_results WHERE turn_id = ?1",
        params![turn_id],
    )?;
    conn.execute(
        "DELETE FROM tool_calls WHERE turn_id = ?1",
        params![turn_id],
    )?;
    Ok(conn.execute("DELETE FROM turns WHERE turn_id = ?1", params![turn_id])?)
}

pub(super) struct InsertTurn<'a> {
    pub(super) turn_id: &'a str,
    pub(super) user_content: &'a str,
    pub(super) user_image_urls: &'a [String],
    pub(super) user_timestamp: &'a str,
    pub(super) assistant_content: &'a str,
    pub(super) assistant_reasoning: Option<&'a str>,
    pub(super) assistant_timestamp: Option<&'a str>,
    pub(super) status: TurnStatus,
    pub(super) tool_reports: &'a [String],
}

/// 用固定 SQL 加载轮次。
///
/// 参数:
/// - `conn`: 数据库连接
/// - `sql`: 查询语句
/// - `params`: 查询参数
///
/// 返回:
/// - 轮次列表
fn load_turns_with_sql<P>(conn: &Connection, sql: &str, params: P) -> Result<Vec<Turn>>
where
    P: rusqlite::Params,
{
    let mut stmt = conn.prepare(sql)?;
    let turns = stmt
        .query_map(params, map_turn)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(turns)
}

/// 从查询行恢复轮次。
///
/// 参数:
/// - `row`: 查询行
///
/// 返回:
/// - 轮次
fn map_turn(row: &Row<'_>) -> rusqlite::Result<Turn> {
    let image_urls_json: String = row.get(3)?;
    let user_image_urls = serde_json::from_str(&image_urls_json).unwrap_or_default();
    let tool_reports_json: String = row.get(9)?;
    let tool_reports = serde_json::from_str(&tool_reports_json).unwrap_or_default();
    let status: String = row.get(8)?;
    Ok(Turn {
        turn_id: row.get(0)?,
        seq: row.get(1)?,
        user_content: row.get(2)?,
        user_image_urls,
        user_timestamp: row.get(4)?,
        assistant_content: row.get(5)?,
        assistant_reasoning: row.get(6)?,
        assistant_timestamp: row.get(7)?,
        status: TurnStatus::from_str(&status),
        tool_reports,
    })
}
