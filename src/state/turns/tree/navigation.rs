use super::builder::{build_tree, path_to_leaf};
use super::model::SessionTree;
use crate::state::turns::repository::{active_leaf_locked, set_active_leaf_locked, ConversationDb};
use anyhow::{bail, Result};

impl ConversationDb {
    /// 读取当前会话树。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 含全部分支的会话树视图
    pub fn session_tree(&self) -> Result<SessionTree> {
        let turns = self.load_turns()?;
        let leaf = {
            let conn = self.conn.lock().unwrap();
            active_leaf_locked(&conn)?
        };
        Ok(build_tree(&turns, leaf))
    }

    /// 返回当前活动分支上的轮次序列。
    ///
    /// 会话历史不再是全部轮次，而是从根到活动叶子的那一条路径；
    /// 其它分支的轮次仍在库里，但不参与本次对话上下文。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 根到活动叶子的轮次，按时间升序
    pub fn active_branch_turns(&self) -> Result<Vec<crate::state::turns::model::Turn>> {
        let turns = self.load_turns()?;
        let leaf = {
            let conn = self.conn.lock().unwrap();
            active_leaf_locked(&conn)?
        };
        let Some(leaf) = leaf else {
            return Ok(Vec::new());
        };
        let path = path_to_leaf(&turns, &leaf);
        if path.is_empty() {
            return Ok(turns);
        }
        // 1. 按路径顺序取出轮次，保证与根到叶的次序一致
        let by_id = turns
            .into_iter()
            .map(|turn| (turn.turn_id.clone(), turn))
            .collect::<std::collections::BTreeMap<_, _>>();
        Ok(path
            .into_iter()
            .filter_map(|turn_id| by_id.get(&turn_id).cloned())
            .collect())
    }

    /// 把活动叶子切换到指定轮次。
    ///
    /// 切换只移动指针，不删除任何轮次：原分支仍可通过树视图回到。
    ///
    /// 参数:
    /// - `turn_id`: 目标轮次
    ///
    /// 返回:
    /// - 切换是否成功；目标轮次不存在时报错
    pub fn switch_active_leaf(&self, turn_id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM turns WHERE turn_id = ?1",
            rusqlite::params![turn_id],
            |row| row.get(0),
        )?;
        if exists == 0 {
            bail!("turn not found: {turn_id}");
        }
        set_active_leaf_locked(&conn, Some(turn_id))
    }

    /// 把活动叶子退回到指定轮次的父轮次。
    ///
    /// 这是撤销的新语义：不删除轮次，只把对话位置往回移一格，
    /// 被退出的分支仍然保留，可以随时切回。
    ///
    /// 参数:
    /// - `turn_id`: 要退出的轮次
    ///
    /// 返回:
    /// - 退回后的活动叶子；已在根部时返回 None
    pub fn move_leaf_to_parent(&self, turn_id: &str) -> Result<Option<String>> {
        let parent: Option<String> = {
            let conn = self.conn.lock().unwrap();
            conn.query_row(
                "SELECT parent_turn_id FROM turns WHERE turn_id = ?1",
                rusqlite::params![turn_id],
                |row| row.get(0),
            )?
        };
        let conn = self.conn.lock().unwrap();
        set_active_leaf_locked(&conn, parent.as_deref())?;
        Ok(parent)
    }
}
