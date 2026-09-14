use super::model::summarize;
use crate::state::turns::model::{TurnStatus, SESSION_ROOT_TURN_ID};
use crate::state::turns::repository::{active_leaf_locked, ConversationDb};
use anyhow::Result;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// 【会话分支】【传输节点】仅包含展示元数据，父子关系通过标识表达。
#[derive(Debug, Serialize)]
pub struct SessionTreeIndexNode {
    pub turn_id: String,
    pub parent_turn_id: Option<String>,
    pub seq: i64,
    pub user_summary: String,
    pub assistant_summary: String,
    pub status: String,
    pub timestamp: String,
}

/// 【会话分支】【传输索引】扁平传输全部分支，避免随对话深度增加序列化栈深度。
#[derive(Debug, Serialize)]
pub struct SessionTreeIndex {
    pub nodes: Vec<SessionTreeIndexNode>,
    pub active_leaf_id: Option<String>,
    pub total_turns: usize,
    pub branch_points: usize,
}

impl ConversationDb {
    /// 【会话分支】【统计查询】直接计算轮次和分叉数量，不读取正文或构造节点。
    /// @returns (轮次数量, 分叉点数量)；无参数
    pub fn session_tree_counts(&self) -> Result<(usize, usize)> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.query_row(
            "SELECT (SELECT COUNT(*) FROM turns),
                    (SELECT COUNT(*) FROM (
                        SELECT child.parent_turn_id
                        FROM turns child JOIN turns parent ON parent.turn_id = child.parent_turn_id
                        GROUP BY child.parent_turn_id HAVING COUNT(*) > 1
                    ))",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?)
    }

    /// 【会话分支】【读取索引】只读取树视图需要的列，不加载图片、工具报告和推理正文。
    /// @returns 包含全部节点、活动位置和分叉数量的扁平索引；无参数
    pub fn session_tree_index(&self) -> Result<SessionTreeIndex> {
        let conn = self.conn.lock().unwrap();
        let transaction = conn.unchecked_transaction()?;
        // 1. 【会话分支】【摘要投影】逐行生成短摘要，避免同时保留全部历史正文
        let mut statement = transaction.prepare(
            "SELECT turn_id, parent_turn_id, seq, user_content, assistant_content, status, user_timestamp
             FROM turns ORDER BY seq ASC",
        )?;
        let mut nodes = statement
            .query_map([], |row| {
                let status: String = row.get(5)?;
                Ok(SessionTreeIndexNode {
                    turn_id: row.get(0)?,
                    parent_turn_id: row.get(1)?,
                    seq: row.get(2)?,
                    user_summary: summarize(&row.get::<_, String>(3)?),
                    assistant_summary: summarize(&row.get::<_, String>(4)?),
                    status: TurnStatus::from_str(&status).as_display_str().to_string(),
                    timestamp: row.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(statement);
        let active_leaf_id =
            active_leaf_locked(&transaction)?.filter(|leaf| leaf != SESSION_ROOT_TURN_ID);
        transaction.commit()?;
        // 2. 【会话分支】【关系校验】缺失的父轮次按根处理，与原树视图保持一致
        let known = nodes
            .iter()
            .map(|node| node.turn_id.clone())
            .collect::<HashSet<_>>();
        let mut children = HashMap::<String, usize>::new();
        for node in &mut nodes {
            node.parent_turn_id = node
                .parent_turn_id
                .take()
                .filter(|parent| known.contains(parent));
            if let Some(parent) = &node.parent_turn_id {
                *children.entry(parent.clone()).or_default() += 1;
            }
        }
        Ok(SessionTreeIndex {
            total_turns: nodes.len(),
            branch_points: children.values().filter(|count| **count > 1).count(),
            nodes,
            active_leaf_id,
        })
    }
}

#[cfg(test)]
mod tests;
