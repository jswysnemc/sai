//! 工具结果图片附件的持久化。
//!
//! read_file 等工具返回的图片与工具结果一样属于上下文：保存后由会话历史投影
//! 在对应工具子轮之后重建附件消息，轮次内压缩与后续轮次都能继续看到图片。

use crate::state::turns::ConversationDb;
use crate::state::StateStore;
use crate::tools::ToolModelAttachment;
use anyhow::Result;
use chrono::Utc;
use rusqlite::{params, Connection};
use std::collections::HashMap;

/// 创建工具图片附件表。
///
/// 参数:
/// - `conn`: SQLite 连接
///
/// 返回:
/// - 建表是否成功
pub(super) fn create_tool_image_table(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS tool_result_images (
            session_id       TEXT NOT NULL,
            turn_id          TEXT NOT NULL,
            provider_call_id TEXT NOT NULL,
            position         INTEGER NOT NULL,
            source           TEXT NOT NULL,
            note             TEXT NOT NULL,
            image_url        TEXT NOT NULL,
            created_at       TEXT NOT NULL,
            PRIMARY KEY(session_id, provider_call_id, position)
        );
        CREATE INDEX IF NOT EXISTS idx_tool_result_images_turn
            ON tool_result_images(session_id, turn_id);",
    )?;
    Ok(())
}

impl StateStore {
    /// 保存一次工具调用返回的全部图片附件。
    ///
    /// 参数:
    /// - `turn_id`: 当前轮次标识
    /// - `provider_call_id`: provider 工具调用标识
    /// - `attachments`: 按返回顺序排列的图片附件
    ///
    /// 返回:
    /// - 写入是否成功
    pub(crate) fn record_tool_result_images(
        &self,
        turn_id: &str,
        provider_call_id: &str,
        attachments: &[ToolModelAttachment],
    ) -> Result<()> {
        if attachments.is_empty() {
            return Ok(());
        }
        let conn = self.conv_db.conn.lock().unwrap();
        let created_at = Utc::now().to_rfc3339();
        for (position, attachment) in attachments.iter().enumerate() {
            conn.execute(
                "INSERT OR REPLACE INTO tool_result_images
                    (session_id, turn_id, provider_call_id, position, source, note, image_url, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    self.session_id,
                    turn_id,
                    provider_call_id,
                    position as i64,
                    attachment.source,
                    attachment.note,
                    attachment.image_url,
                    created_at,
                ],
            )?;
        }
        Ok(())
    }
}

/// 读取指定轮次全部工具调用的图片附件。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `turn_id`: 轮次标识
///
/// 返回:
/// - 以 provider 工具调用标识分组、组内按返回顺序排列的附件
pub(in crate::state) fn load_tool_result_images_for_turn(
    db: &ConversationDb,
    session_id: &str,
    turn_id: &str,
) -> Result<HashMap<String, Vec<ToolModelAttachment>>> {
    let conn = db.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT provider_call_id, source, note, image_url FROM tool_result_images
         WHERE session_id = ?1 AND turn_id = ?2
         ORDER BY provider_call_id, position",
    )?;
    let rows = stmt.query_map(params![session_id, turn_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            ToolModelAttachment::new(
                row.get::<_, String>(3)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ),
        ))
    })?;
    let mut grouped: HashMap<String, Vec<ToolModelAttachment>> = HashMap::new();
    for row in rows {
        let (call_id, attachment) = row?;
        grouped.entry(call_id).or_default().push(attachment);
    }
    Ok(grouped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::tool_history::schema::create_tool_history_tables;

    /// 图片按调用分组并保持返回顺序。
    #[test]
    fn records_and_loads_images_grouped_by_call() {
        let temp = tempfile::tempdir().unwrap();
        let db = ConversationDb::open(temp.path()).unwrap();
        create_tool_history_tables(&db.conn.lock().unwrap()).unwrap();
        let store = StateStore {
            plugin_state_root: None,
            base_state_dir: temp.path().to_path_buf(),
            session_id: "default".to_string(),
            state_dir: temp.path().to_path_buf(),
            conv_db: std::sync::Arc::new(db),
        };
        store
            .record_tool_result_images(
                "turn_1",
                "call_1",
                &[
                    ToolModelAttachment::new("data:image/png;base64,AA", "a.png", "note a"),
                    ToolModelAttachment::new("data:image/png;base64,BB", "b.png", "note b"),
                ],
            )
            .unwrap();

        let images = load_tool_result_images_for_turn(&store.conv_db, "default", "turn_1").unwrap();

        let call = &images["call_1"];
        assert_eq!(call.len(), 2);
        assert_eq!(call[0].source, "a.png");
        assert_eq!(call[1].note, "note b");
        assert!(
            load_tool_result_images_for_turn(&store.conv_db, "default", "turn_2")
                .unwrap()
                .is_empty()
        );
    }
}
