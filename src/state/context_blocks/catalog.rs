use super::model::{Block, Candidate, Catalog};
use super::schema;
use crate::state::{StateStore, ToolCallStatus};
use anyhow::Result;
use std::collections::HashSet;

/// 【上下文】【候选保护】当前工具轮次末尾至少保留的调用数
const KEEP_RECENT_CALLS: usize = 4;

impl StateStore {
    /// 【上下文】【候选查询】列出当前分支的结果及活动摘要块，避免输出无界列表
    /// 参数: offset 为候选偏移，limit 为页大小，block_offset 为摘要偏移；返回候选、保护原因与修订号
    pub(crate) fn context_block_catalog(
        &self,
        offset: usize,
        limit: usize,
        block_offset: usize,
    ) -> Result<Catalog> {
        let mut candidates = self.context_block_candidates()?;
        let total = candidates.len();
        let start = offset.min(total);
        let end = start.saturating_add(limit.clamp(1, 50)).min(total);
        let candidates = candidates.drain(start..end).collect();
        let conn = self.conv_db.conn.lock().unwrap();
        let revision = schema::revision(&conn, &self.session_id)?;
        drop(conn);
        let mut blocks = self.visible_context_blocks()?;
        let total_blocks = blocks.len();
        let block_start = block_offset.min(total_blocks);
        let block_end = block_start.saturating_add(5).min(total_blocks);
        let blocks = blocks.drain(block_start..block_end).collect();
        Ok(Catalog {
            revision,
            candidates,
            total,
            next_offset: (end < total).then_some(end),
            blocks,
            total_blocks,
            next_block_offset: (block_end < total_blocks).then_some(block_end),
        })
    }

    /// 【上下文】【候选保护】从活动分支读取工具结果，保护近期调用、错误与管理工具
    /// 参数: 无；返回完整候选列表，正文只供内部归档与预算检查
    pub(super) fn context_block_candidates(&self) -> Result<Vec<Candidate>> {
        let turns = self.conv_db.active_branch_turns()?;
        let boundary = self.running_turn_compaction_boundary()?;
        let archived = {
            let conn = self.conv_db.conn.lock().unwrap();
            let mut stmt = conn
                .prepare("SELECT message_id FROM context_block_messages WHERE session_id = ?1")?;
            let rows = stmt.query_map([&self.session_id], |row| row.get::<_, String>(0))?;
            rows.collect::<std::result::Result<HashSet<_>, _>>()?
        };
        let mut exchanges = Vec::new();
        for turn in turns {
            for exchange in crate::state::tool_history::load_tool_exchanges_for_turn(
                &self.conv_db,
                &self.session_id,
                &turn.turn_id,
            )? {
                if boundary
                    .as_ref()
                    .is_some_and(|(id, count)| id == &turn.turn_id && exchange.call.seq <= *count)
                {
                    continue;
                }
                exchanges.push(exchange);
            }
        }
        let recent_start = exchanges.len().saturating_sub(KEEP_RECENT_CALLS);
        let mut candidates = Vec::new();
        for (index, exchange) in exchanges.into_iter().enumerate() {
            let Some(result) = exchange.result else {
                continue;
            };
            let tool = exchange
                .call
                .display_tool_name
                .unwrap_or(exchange.call.tool_name);
            let visible = exchange
                .replacement
                .as_ref()
                .map(|item| item.replacement.clone())
                .unwrap_or_else(|| result.result_preview.clone());
            let protected_reason = if archived.contains(&exchange.call.provider_call_id) {
                Some("already_compressed")
            } else if !result.ok || exchange.call.status != ToolCallStatus::Completed {
                Some("error_or_unfinished")
            } else if crate::tools::context_blocks::is_context_tool(&tool) {
                Some("context_management")
            } else if index >= recent_start {
                Some("recent_working_set")
            } else {
                None
            };
            candidates.push(Candidate {
                message_id: exchange.call.provider_call_id,
                turn_id: exchange.call.turn_id,
                tool,
                tokens: crate::token_estimate::estimate_tokens(&visible),
                preview: visible.chars().take(160).collect(),
                eligible: protected_reason.is_none(),
                protected_reason: protected_reason.map(str::to_string),
                original_preview: result.result_preview,
                original_chars: result.original_chars,
                result_ref: exchange
                    .replacement
                    .map(|item| item.result_ref)
                    .filter(|reference| !reference.is_empty())
                    .or(result.result_ref),
                arguments: exchange
                    .call
                    .display_arguments
                    .unwrap_or(exchange.call.arguments),
                visible,
            });
        }
        Ok(candidates)
    }

    /// 【上下文】【分支隔离】读取当前分支或其权威摘要覆盖的归档引用
    /// 参数: 无；返回允许搜索及回读的消息引用集合
    pub(super) fn visible_context_message_ids(&self) -> Result<HashSet<String>> {
        let turns = self
            .conv_db
            .active_branch_turns()?
            .into_iter()
            .map(|turn| turn.turn_id)
            .collect::<Vec<_>>();
        let conn = self.conv_db.conn.lock().unwrap();
        let checkpoint =
            crate::state::checkpoints::load_latest_checkpoint(&conn)?.map(|item| item.id);
        let mut stmt = conn.prepare(
            "SELECT message_id FROM context_block_messages WHERE session_id = ?1
             AND (turn_id IN (SELECT value FROM json_each(?2)) OR checkpoint_id = ?3)",
        )?;
        let rows = stmt.query_map(
            rusqlite::params![self.session_id, serde_json::to_string(&turns)?, checkpoint],
            |row| row.get(0),
        )?;
        Ok(rows.collect::<std::result::Result<HashSet<_>, _>>()?)
    }

    /// 【上下文】【分支隔离】只有所有成员可见时才展示一个摘要，防止混入另一分支内容
    /// 参数: 无；返回当前分支可用摘要块
    pub(super) fn visible_context_blocks(&self) -> Result<Vec<Block>> {
        let allowed = self.visible_context_message_ids()?;
        let conn = self.conv_db.conn.lock().unwrap();
        let mut stmt =
            conn.prepare("SELECT body FROM context_blocks WHERE session_id = ?1 ORDER BY rowid")?;
        let rows = stmt.query_map([&self.session_id], |row| row.get::<_, String>(0))?;
        let mut blocks = Vec::new();
        for row in rows {
            let block: Block = serde_json::from_str(&row?)?;
            if block.message_ids.iter().all(|id| allowed.contains(id)) {
                blocks.push(block);
            }
        }
        Ok(blocks)
    }
}
