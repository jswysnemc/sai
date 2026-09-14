use super::super::model::{Turn, SESSION_ROOT_TURN_ID};
use super::super::repository::{active_leaf_locked, ConversationDb};
use super::super::rows::map_turn;
use anyhow::Result;
use rusqlite::OptionalExtension;
use std::collections::HashSet;

impl ConversationDb {
    /// 【会话历史】【限量读取】从活动叶子沿父指针读取最近轮次，不解码其他正文。
    /// @param limit 最大轮次数量；零表示不读取历史
    /// @returns 当前分支最近轮次，顺序与完整分支一致
    pub(in crate::state) fn recent_active_branch_turns(&self, limit: usize) -> Result<Vec<Turn>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let conn = self.conn.lock().unwrap();
        // 1. 【会话历史】【一致性】叶子与父链在同一个只读事务快照内读取
        let transaction = conn.unchecked_transaction()?;
        let mut cursor = active_leaf_locked(&transaction)?;
        let mut seen = HashSet::new();
        let mut turns = Vec::new();
        let mut statement = transaction.prepare(
            "SELECT turn_id, seq, user_content, user_image_urls, user_timestamp, assistant_content,
                    assistant_reasoning, assistant_timestamp, status, tool_reports, duration_ms,
                    parent_turn_id, model, error
             FROM turns WHERE turn_id = ?1",
        )?;
        // 2. 【会话历史】【窗口边界】每次主键查询只读取一个需要展示的轮次
        while let Some(id) = cursor {
            if turns.len() == limit || id == SESSION_ROOT_TURN_ID || !seen.insert(id.clone()) {
                break;
            }
            let Some(turn) = statement.query_row([&id], map_turn).optional()? else {
                break;
            };
            cursor = turn.parent_turn_id.clone();
            turns.push(turn);
        }
        drop(statement);
        transaction.commit()?;
        // 3. 【会话历史】【展示顺序】从叶子回溯的结果恢复为从旧到新的顺序
        turns.reverse();
        Ok(turns)
    }
}
