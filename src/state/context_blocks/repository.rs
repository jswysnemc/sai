use super::model::{replacement, Block, CompressRequest};
use super::schema;
use crate::state::StateStore;
use anyhow::{bail, ensure, Context, Result};
use rusqlite::{params, OptionalExtension};
use std::collections::{HashMap, HashSet};

/// 【上下文】【归档预算】单次提交最多归档的原文总字节数
const MAX_ARCHIVE_BYTES: usize = 16 * 1024 * 1024;

impl StateStore {
    /// 【上下文】【局部压缩】先读取全部原文，再原子提交归档和摘要，原始工具记录保持完整
    /// 参数: request 为候选引用、摘要及修订号；返回已提交块，完全相同的重复请求返回原块
    pub(crate) fn compress_context_block(&self, request: &CompressRequest) -> Result<Block> {
        ensure!(
            !request.message_ids.is_empty() && request.message_ids.len() <= 32,
            "select between 1 and 32 message ids"
        );
        ensure!(
            !request.summary.trim().is_empty() && request.summary.chars().count() <= 6000,
            "summary must contain 1..6000 characters"
        );
        ensure!(
            !request.topic.trim().is_empty() && request.topic.chars().count() <= 120,
            "topic must contain 1..120 characters"
        );
        let unique = request.message_ids.iter().collect::<HashSet<_>>();
        ensure!(
            unique.len() == request.message_ids.len(),
            "duplicate message ids"
        );
        let request_hash = blake3::hash(&serde_json::to_vec(&(
            &request.message_ids,
            request.summary.trim(),
            request.topic.trim(),
        ))?)
        .to_hex()
        .to_string();
        // 1. 【上下文】【重复提交】并发提交落盘后重查同一请求，避免成功重试被当成冲突
        if let Some(block) = self.context_block_by_request(&request_hash)? {
            return Ok(block);
        }
        match self.commit_context_block(request, &request_hash) {
            Ok(block) => Ok(block),
            Err(error) => match self.context_block_by_request(&request_hash)? {
                Some(block) => Ok(block),
                None => Err(error),
            },
        }
    }

    /// 【上下文】【重复提交】按请求指纹读取当前分支中已完成的块
    /// 参数: request_hash 为规范化请求指纹；返回可见块或空值
    fn context_block_by_request(&self, request_hash: &str) -> Result<Option<Block>> {
        let allowed = self.visible_context_message_ids()?;
        let conn = self.conv_db.conn.lock().unwrap();
        let body: Option<String> = conn
            .query_row(
                "SELECT body FROM context_blocks WHERE session_id = ?1 AND request_hash = ?2",
                params![self.session_id, request_hash],
                |row| row.get(0),
            )
            .optional()?;
        let block: Option<Block> = body.map(|body| serde_json::from_str(&body)).transpose()?;
        Ok(block.filter(|block| block.message_ids.iter().all(|id| allowed.contains(id))))
    }

    /// 【上下文】【原子提交】校验来源、归档原文并一次写入摘要及修订号
    /// 参数: request 为已校验请求，request_hash 为指纹；返回新块
    fn commit_context_block(&self, request: &CompressRequest, request_hash: &str) -> Result<Block> {
        let leaf = {
            let conn = self.conv_db.conn.lock().unwrap();
            schema::leaf(&conn)?
        };
        let mut candidates = self
            .context_block_candidates()?
            .into_iter()
            .map(|item| (item.message_id.clone(), item))
            .collect::<HashMap<_, _>>();
        let mut chosen = Vec::new();
        for id in &request.message_ids {
            let candidate = candidates
                .remove(id)
                .with_context(|| format!("unknown or inactive message id: {id}"))?;
            ensure!(
                candidate.eligible,
                "protected message {id}: {}",
                candidate.protected_reason.as_deref().unwrap_or("protected")
            );
            chosen.push(candidate);
        }
        let before_tokens = chosen.iter().map(|item| item.tokens).sum();
        let mut block = Block {
            block_id: format!("cb_{}", uuid::Uuid::new_v4().simple()),
            topic: request.topic.trim().into(),
            summary: request.summary.trim().into(),
            message_ids: request.message_ids.clone(),
            before_tokens,
            after_tokens: 0,
            revision: request.expected_revision.saturating_add(1),
        };
        block.after_tokens = block
            .message_ids
            .iter()
            .map(|id| crate::token_estimate::estimate_tokens(&replacement(&block, id)))
            .sum();
        ensure!(
            block.before_tokens >= block.after_tokens.saturating_add(128),
            "compression must reclaim at least 128 tokens after reference overhead"
        );

        // 2. 【上下文】【原文归档】任何一个引用缺失或超出预算，都不提交摘要
        let reader = self.tool_result_ref_reader()?;
        let mut remaining = MAX_ARCHIVE_BYTES;
        let mut originals = Vec::new();
        for candidate in &chosen {
            let original = if let Some(reference) = candidate
                .result_ref
                .as_deref()
                .filter(|reference| !reference.is_empty())
            {
                reader
                    .read_with_limit(reference, remaining)?
                    .context("original output exceeds archive byte budget")?
            } else {
                ensure!(
                    candidate.original_preview.chars().count() >= candidate.original_chars,
                    "full original unavailable for {}; refusing to archive a truncated preview",
                    candidate.message_id
                );
                candidate.original_preview.clone()
            };
            ensure!(
                original.len() <= remaining,
                "original output exceeds archive byte budget"
            );
            remaining -= original.len();
            originals.push(original);
        }

        // 3. 【上下文】【原子提交】修订号、分支和来源都必须与选择时一致
        let mut conn = self.conv_db.conn.lock().unwrap();
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        ensure!(
            schema::leaf(&tx)? == leaf,
            "active branch changed; query context_status again"
        );
        ensure!(
            schema::revision(&tx, &self.session_id)? == request.expected_revision,
            "stale context revision; query context_status again"
        );
        for candidate in &chosen {
            let current: Option<String> = tx.query_row(
                "SELECT COALESCE(p.replacement, r.result_preview) FROM tool_results r
                 LEFT JOIN tool_output_replacements p ON p.provider_call_id = r.provider_call_id AND p.session_id = r.session_id
                 WHERE r.session_id = ?1 AND r.provider_call_id = ?2 AND r.ok = 1",
                params![self.session_id, candidate.message_id], |row| row.get(0),
            ).optional()?;
            ensure!(
                current.as_deref() == Some(candidate.visible.as_str()),
                "source changed; query context_status again"
            );
            let exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM context_block_messages WHERE session_id = ?1 AND message_id = ?2)",
                params![self.session_id, candidate.message_id], |row| row.get(0),
            )?;
            if exists {
                bail!("message {} is already compressed", candidate.message_id);
            }
        }
        tx.execute(
            "INSERT INTO context_blocks(block_id, session_id, request_hash, body) VALUES (?1, ?2, ?3, ?4)",
            params![block.block_id, self.session_id, request_hash, serde_json::to_string(&block)?],
        )?;
        for (candidate, original) in chosen.iter().zip(originals) {
            tx.execute(
                "INSERT INTO context_block_messages(session_id, message_id, block_id, turn_id, tool, arguments, original)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![self.session_id, candidate.message_id, block.block_id, candidate.turn_id, candidate.tool, candidate.arguments, original],
            )?;
        }
        tx.execute(
            "INSERT INTO context_block_revision(session_id, revision) VALUES (?1, ?2)
             ON CONFLICT(session_id) DO UPDATE SET revision = excluded.revision",
            params![self.session_id, block.revision],
        )?;
        tx.commit()?;
        Ok(block)
    }
}
