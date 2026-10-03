use super::model::{RestoredMessage, SearchHit};
use crate::state::StateStore;
use anyhow::{ensure, Context, Result};
use rusqlite::{params, OptionalExtension};

impl StateStore {
    /// 【上下文】【精确回读】按稳定引用读取原始输出，分页不改变任何已压缩历史
    /// 参数: message_id 为结果引用，offset/limit 为字符偏移和页长；返回正文页与下一页偏移
    pub(crate) fn restore_context_message(
        &self,
        message_id: &str,
        offset: usize,
        limit: usize,
    ) -> Result<RestoredMessage> {
        ensure!(
            self.visible_context_message_ids()?.contains(message_id),
            "message is not archived on the active branch"
        );
        let conn = self.conv_db.conn.lock().unwrap();
        let (tool, arguments, original): (String, String, String) = conn
            .query_row(
                "SELECT tool, arguments, original FROM context_block_messages
             WHERE session_id = ?1 AND message_id = ?2",
                params![self.session_id, message_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?
            .context("archived message not found")?;
        // 1. 【上下文】【精确回读】按 Rust 字符分页，保留嵌入的空字符及多字节文本
        let total = original.chars().count();
        ensure!(offset <= total, "offset exceeds original length");
        let content: String = original
            .chars()
            .skip(offset)
            .take(limit.clamp(1, 8000))
            .collect();
        let end = offset.saturating_add(content.chars().count());
        Ok(RestoredMessage {
            message_id: message_id.into(),
            tool,
            arguments: arguments.chars().take(2000).collect(),
            content,
            offset,
            total_chars: total,
            next_offset: (end < total).then_some(end),
        })
    }

    /// 【上下文】【历史检索】在原文、摘要和工具名称中搜索，结果只暴露当前分支可读引用
    /// 参数: query 为非空关键词，limit 为结果上限；返回有界预览
    pub(crate) fn search_context_blocks(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchHit>> {
        let query = query.trim();
        ensure!(
            !query.is_empty() && query.chars().count() <= 200,
            "query must contain 1..200 characters"
        );
        let allowed = self.visible_context_message_ids()?;
        let allowed_blocks = self
            .visible_context_blocks()?
            .into_iter()
            .map(|block| block.block_id)
            .collect::<Vec<_>>();
        let conn = self.conv_db.conn.lock().unwrap();
        // 1. 【上下文】【历史检索】在数据库内截取命中附近内容，避免把全部归档读入上下文
        let mut stmt = conn.prepare(
            "SELECT m.block_id, m.message_id, m.tool,
                substr(m.original, max(1, instr(lower(m.original), lower(?2)) - 100), 400)
             FROM context_block_messages m JOIN context_blocks b ON b.block_id = m.block_id
             WHERE m.session_id = ?1 AND m.message_id IN (SELECT value FROM json_each(?3))
               AND (instr(lower(m.original), lower(?2)) > 0 OR instr(lower(m.tool), lower(?2)) > 0
                    OR (b.block_id IN (SELECT value FROM json_each(?5))
                        AND (instr(lower(json_extract(b.body, '$.topic')), lower(?2)) > 0
                            OR instr(lower(json_extract(b.body, '$.summary')), lower(?2)) > 0)))
             ORDER BY m.rowid DESC LIMIT ?4",
        )?;
        let rows = stmt.query_map(
            params![
                self.session_id,
                query,
                serde_json::to_string(&allowed)?,
                limit.clamp(1, 20),
                serde_json::to_string(&allowed_blocks)?
            ],
            |row| {
                Ok(SearchHit {
                    block_id: row.get(0)?,
                    message_id: row.get(1)?,
                    tool: row.get(2)?,
                    snippet: row.get(3)?,
                })
            },
        )?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }
}
