use super::SuperAgentStore;
use crate::super_agent::model::{
    new_id, now_millis, GroupMessage, GroupThread, MessageKind, MessageSenderKind,
};
use anyhow::Result;
use rusqlite::{params, OptionalExtension};

/// 写入群组消息的入参载荷。
#[derive(Debug, Clone)]
pub struct NewGroupMessage {
    pub group_id: String,
    pub thread_id: String,
    pub sender_kind: MessageSenderKind,
    pub sender_id: String,
    pub sender_name: String,
    pub recipient_id: Option<String>,
    pub recipient_name: Option<String>,
    pub kind: MessageKind,
    pub channel_source: Option<String>,
    pub content: String,
    pub summary: Option<String>,
    pub related_task_id: Option<String>,
    pub related_report_id: Option<String>,
}

impl SuperAgentStore {
    /// 创建新的群组话题 Thread。
    ///
    /// 参数:
    /// - `group_id`: 群组标识
    /// - `title`: 话题标题
    /// - `channel`: 来源渠道名
    /// - `external_ref`: 可选外部会话/话题引用标识
    ///
    /// 返回:
    /// - 新建线程记录
    pub fn create_thread(
        &self,
        group_id: &str,
        title: &str,
        channel: &str,
        external_ref: Option<&str>,
    ) -> Result<GroupThread> {
        let now = now_millis();
        let id = new_id("thr");
        let clean_title = title.trim();
        let thread_title = if clean_title.is_empty() {
            "Untitled Thread"
        } else {
            clean_title
        };
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO super_threads
             (id, group_id, title, channel, external_ref, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, 'open', ?6, ?7)",
            params![id, group_id, thread_title, channel.trim(), external_ref, now, now],
        )?;
        Ok(GroupThread {
            id,
            group_id: group_id.to_string(),
            title: thread_title.to_string(),
            channel: channel.trim().to_string(),
            external_ref: external_ref.map(ToOwned::to_owned),
            status: "open".to_string(),
            created_at: now,
            updated_at: now,
        })
    }

    /// 查找或按外部渠道引用自动建立 Thread（保证同一外部对话连续）。
    ///
    /// 参数:
    /// - `group_id`: 群组标识
    /// - `channel`: 渠道名
    /// - `external_ref`: 外部会话或话题 ID
    /// - `title`: 新建时使用的默认标题
    ///
    /// 返回:
    /// - 命中或新建的 Thread
    pub fn ensure_channel_thread(
        &self,
        group_id: &str,
        channel: &str,
        external_ref: &str,
        title: &str,
    ) -> Result<GroupThread> {
        let conn = self.conn()?;
        if let Some(existing) = conn
            .query_row(
                "SELECT id, group_id, title, channel, external_ref, status, created_at, updated_at
                 FROM super_threads
                 WHERE group_id = ?1 AND channel = ?2 AND external_ref = ?3
                 ORDER BY updated_at DESC LIMIT 1",
                params![group_id, channel, external_ref],
                map_thread,
            )
            .optional()?
        {
            return Ok(existing);
        }
        drop(conn);
        self.create_thread(group_id, title, channel, Some(external_ref))
    }

    /// 按 ID 读取单个 Thread。
    ///
    /// 参数:
    /// - `thread_id`: 线程标识
    ///
    /// 返回:
    /// - 线程记录
    pub fn get_thread(&self, thread_id: &str) -> Result<Option<GroupThread>> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT id, group_id, title, channel, external_ref, status, created_at, updated_at
             FROM super_threads WHERE id = ?1",
            params![thread_id],
            map_thread,
        )
        .optional()
        .map_err(Into::into)
    }

    /// 列出群组内全部 Thread。
    ///
    /// 参数:
    /// - `group_id`: 群组标识
    ///
    /// 返回:
    /// - 线程列表
    pub fn list_threads(&self, group_id: &str) -> Result<Vec<GroupThread>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, group_id, title, channel, external_ref, status, created_at, updated_at
             FROM super_threads
             WHERE group_id = ?1
             ORDER BY updated_at DESC, created_at ASC",
        )?;
        let rows = stmt.query_map(params![group_id], map_thread)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// 追加一条群组消息并刷新所属群组与 Thread 活跃时间。
    ///
    /// 参数:
    /// - `input`: 新消息载荷
    ///
    /// 返回:
    /// - 已分配自增 seq 的群组消息
    pub fn append_message(&self, input: NewGroupMessage) -> Result<GroupMessage> {
        let now = now_millis();
        let id = new_id("msg");
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO super_messages
             (id, group_id, thread_id, sender_kind, sender_id, sender_name, recipient_id,
              recipient_name, kind, channel_source, content, summary, related_task_id,
              related_report_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                id,
                input.group_id,
                input.thread_id,
                input.sender_kind.as_str(),
                input.sender_id,
                input.sender_name,
                input.recipient_id,
                input.recipient_name,
                input.kind.as_str(),
                input.channel_source,
                input.content,
                input.summary,
                input.related_task_id,
                input.related_report_id,
                now,
            ],
        )?;
        let seq = tx.last_insert_rowid();
        tx.execute(
            "UPDATE super_groups SET updated_at = ?2 WHERE id = ?1",
            params![input.group_id, now],
        )?;
        tx.execute(
            "UPDATE super_threads SET updated_at = ?2 WHERE id = ?1",
            params![input.thread_id, now],
        )?;
        tx.commit()?;
        Ok(GroupMessage {
            id,
            seq,
            group_id: input.group_id,
            thread_id: input.thread_id,
            sender_kind: input.sender_kind,
            sender_id: input.sender_id,
            sender_name: input.sender_name,
            recipient_id: input.recipient_id,
            recipient_name: input.recipient_name,
            kind: input.kind,
            channel_source: input.channel_source,
            content: input.content,
            summary: input.summary,
            related_task_id: input.related_task_id,
            related_report_id: input.related_report_id,
            created_at: now,
        })
    }

    /// 读取群组最近消息列表（按 seq 升序返回）。
    ///
    /// 参数:
    /// - `group_id`: 群组标识
    /// - `limit`: 最大返回条数
    /// - `after_seq`: 可选增量游标，仅返回大于该 seq 的消息
    ///
    /// 返回:
    /// - 按时间顺序排列的群组消息
    pub fn list_messages(
        &self,
        group_id: &str,
        limit: usize,
        after_seq: Option<i64>,
    ) -> Result<Vec<GroupMessage>> {
        let conn = self.conn()?;
        let cap = limit.clamp(1, 500) as i64;
        if let Some(after) = after_seq {
            let mut stmt = conn.prepare(
                "SELECT id, seq, group_id, thread_id, sender_kind, sender_id, sender_name,
                        recipient_id, recipient_name, kind, channel_source, content, summary,
                        related_task_id, related_report_id, created_at
                 FROM super_messages
                 WHERE group_id = ?1 AND seq > ?2
                 ORDER BY seq ASC
                 LIMIT ?3",
            )?;
            let rows = stmt.query_map(params![group_id, after, cap], map_message)?;
            return rows
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(Into::into);
        }
        let mut stmt = conn.prepare(
            "SELECT id, seq, group_id, thread_id, sender_kind, sender_id, sender_name,
                    recipient_id, recipient_name, kind, channel_source, content, summary,
                    related_task_id, related_report_id, created_at
             FROM (
                SELECT * FROM super_messages
                WHERE group_id = ?1
                ORDER BY seq DESC
                LIMIT ?2
             )
             ORDER BY seq ASC",
        )?;
        let rows = stmt.query_map(params![group_id, cap], map_message)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// 读取指定 Thread 下的最近消息（供 Chain of Recall 按需展开）。
    ///
    /// 参数:
    /// - `thread_id`: 线程标识
    /// - `limit`: 最大条数
    ///
    /// 返回:
    /// - 线程内消息列表
    pub fn list_thread_messages(&self, thread_id: &str, limit: usize) -> Result<Vec<GroupMessage>> {
        let conn = self.conn()?;
        let cap = limit.clamp(1, 200) as i64;
        let mut stmt = conn.prepare(
            "SELECT id, seq, group_id, thread_id, sender_kind, sender_id, sender_name,
                    recipient_id, recipient_name, kind, channel_source, content, summary,
                    related_task_id, related_report_id, created_at
             FROM (
                SELECT * FROM super_messages
                WHERE thread_id = ?1
                ORDER BY seq DESC
                LIMIT ?2
             )
             ORDER BY seq ASC",
        )?;
        let rows = stmt.query_map(params![thread_id, cap], map_message)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// 按消息 ID 读取单条完整消息（供 Spill 内容续读）。
    ///
    /// 参数:
    /// - `message_id`: 消息标识
    ///
    /// 返回:
    /// - 消息记录
    pub fn get_message(&self, message_id: &str) -> Result<Option<GroupMessage>> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT id, seq, group_id, thread_id, sender_kind, sender_id, sender_name,
                    recipient_id, recipient_name, kind, channel_source, content, summary,
                    related_task_id, related_report_id, created_at
             FROM super_messages WHERE id = ?1",
            params![message_id],
            map_message,
        )
        .optional()
        .map_err(Into::into)
    }
}

/// 将 SQLite 行映射为 GroupThread。
///
/// 参数:
/// - `row`: 查询结果行
///
/// 返回:
/// - GroupThread 实体
fn map_thread(row: &rusqlite::Row<'_>) -> rusqlite::Result<GroupThread> {
    Ok(GroupThread {
        id: row.get(0)?,
        group_id: row.get(1)?,
        title: row.get(2)?,
        channel: row.get(3)?,
        external_ref: row.get(4)?,
        status: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

/// 将 SQLite 行映射为 GroupMessage。
///
/// 参数:
/// - `row`: 查询结果行
///
/// 返回:
/// - GroupMessage 实体
fn map_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<GroupMessage> {
    let sender_kind: String = row.get(4)?;
    let kind: String = row.get(9)?;
    Ok(GroupMessage {
        id: row.get(0)?,
        seq: row.get(1)?,
        group_id: row.get(2)?,
        thread_id: row.get(3)?,
        sender_kind: MessageSenderKind::parse(&sender_kind),
        sender_id: row.get(5)?,
        sender_name: row.get(6)?,
        recipient_id: row.get(7)?,
        recipient_name: row.get(8)?,
        kind: MessageKind::parse(&kind),
        channel_source: row.get(10)?,
        content: row.get(11)?,
        summary: row.get(12)?,
        related_task_id: row.get(13)?,
        related_report_id: row.get(14)?,
        created_at: row.get(15)?,
    })
}
