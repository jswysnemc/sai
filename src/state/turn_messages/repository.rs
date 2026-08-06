use super::{NewTurnMessage, TurnMessageKind, TurnMessageRecord};
use crate::state::{ConversationDb, StateStore};
use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Row};

impl StateStore {
    /// 记录同一对话轮次中的一条模型间隙消息。
    ///
    /// 参数:
    /// - `message`: 待写入消息及其工具顺序边界
    ///
    /// 返回:
    /// - 已持久化消息
    pub(crate) fn record_turn_message(&self, message: NewTurnMessage) -> Result<TurnMessageRecord> {
        insert_turn_message(&self.conv_db, message)
    }

    /// 删除尚未成功交给模型的轮次内消息。
    ///
    /// 参数:
    /// - `message_id`: 消息标识
    ///
    /// 返回:
    /// - 是否删除了记录
    pub(crate) fn remove_turn_message(&self, message_id: &str) -> Result<bool> {
        delete_turn_message(&self.conv_db, message_id)
    }

    /// 读取指定轮次的全部间隙消息。
    ///
    /// 参数:
    /// - `turn_id`: 对话轮次标识
    ///
    /// 返回:
    /// - 按写入顺序排列的消息
    pub(crate) fn turn_messages(&self, turn_id: &str) -> Result<Vec<TurnMessageRecord>> {
        load_turn_messages_for_turn(&self.conv_db, turn_id)
    }
}

/// 插入一条轮次内消息。
///
/// 参数:
/// - `db`: 对话数据库
/// - `message`: 待写入消息
///
/// 返回:
/// - 已写入消息
fn insert_turn_message(db: &ConversationDb, message: NewTurnMessage) -> Result<TurnMessageRecord> {
    let conn = db.conn.lock().unwrap();
    let seq: i64 = conn.query_row(
        "SELECT COALESCE(MAX(seq), 0) + 1 FROM turn_messages WHERE turn_id = ?1",
        params![message.turn_id],
        |row| row.get(0),
    )?;
    let id = format!("turn_message_{}", uuid::Uuid::new_v4().simple());
    let created_at = Utc::now().to_rfc3339();
    let image_urls = serde_json::to_string(&message.image_urls)?;
    conn.execute(
        "INSERT INTO turn_messages (
            id, turn_id, seq, after_tool_seq, kind, model_content,
            display_content, reasoning, image_urls, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            id,
            message.turn_id,
            seq,
            message.after_tool_seq as i64,
            message.kind.as_str(),
            message.model_content,
            message.display_content,
            message.reasoning,
            image_urls,
            created_at,
        ],
    )?;
    Ok(TurnMessageRecord {
        id,
        turn_id: message.turn_id,
        seq: seq as usize,
        after_tool_seq: message.after_tool_seq,
        kind: message.kind,
        model_content: message.model_content,
        display_content: message.display_content,
        reasoning: message.reasoning,
        image_urls: message.image_urls,
        created_at,
    })
}

/// 删除一条轮次内消息。
///
/// 参数:
/// - `db`: 对话数据库
/// - `message_id`: 消息标识
///
/// 返回:
/// - 是否删除了记录
fn delete_turn_message(db: &ConversationDb, message_id: &str) -> Result<bool> {
    Ok(db.conn.lock().unwrap().execute(
        "DELETE FROM turn_messages WHERE id = ?1",
        params![message_id],
    )? > 0)
}

/// 读取一个轮次中的全部间隙消息。
///
/// 参数:
/// - `db`: 对话数据库
/// - `turn_id`: 对话轮次标识
///
/// 返回:
/// - 按写入顺序排列的消息
pub(in crate::state) fn load_turn_messages_for_turn(
    db: &ConversationDb,
    turn_id: &str,
) -> Result<Vec<TurnMessageRecord>> {
    let conn = db.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, turn_id, seq, after_tool_seq, kind, model_content,
                display_content, reasoning, image_urls, created_at
         FROM turn_messages
         WHERE turn_id = ?1
         ORDER BY seq ASC",
    )?;
    let messages = stmt
        .query_map(params![turn_id], map_turn_message)?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(messages)
}

/// 从数据库行恢复轮次内消息。
///
/// 参数:
/// - `row`: 查询结果行
///
/// 返回:
/// - 轮次内消息
fn map_turn_message(row: &Row<'_>) -> rusqlite::Result<TurnMessageRecord> {
    let kind: String = row.get(4)?;
    let image_urls: String = row.get(8)?;
    Ok(TurnMessageRecord {
        id: row.get(0)?,
        turn_id: row.get(1)?,
        seq: row.get::<_, i64>(2)?.max(0) as usize,
        after_tool_seq: row.get::<_, i64>(3)?.max(0) as usize,
        kind: TurnMessageKind::from_str(&kind),
        model_content: row.get(5)?,
        display_content: row.get(6)?,
        reasoning: row.get(7)?,
        image_urls: serde_json::from_str(&image_urls).unwrap_or_default(),
        created_at: row.get(9)?,
    })
}
