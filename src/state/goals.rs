use super::StateStore;
use crate::goal::{Goal, GoalStatus, GoalStore};
use anyhow::Result;
use std::path::PathBuf;

impl StateStore {
    /// 返回当前会话目标状态文件。
    ///
    /// 返回:
    /// - Goal JSON 文件路径
    pub(crate) fn goal_file(&self) -> PathBuf {
        self.state_dir.join("goal.json")
    }

    /// 读取当前会话目标。
    ///
    /// 返回:
    /// - 当前目标；尚未设置时返回空
    pub(crate) fn goal(&self) -> Result<Option<Goal>> {
        GoalStore::new(self.goal_file()).get()
    }

    /// 创建或替换当前会话目标。
    ///
    /// 参数:
    /// - `objective`: 目标文本
    /// - `token_budget`: 可选 Token 预算
    /// - `replace_unfinished`: 是否允许替换未完成目标
    ///
    /// 返回:
    /// - 新目标
    pub(crate) fn replace_goal(
        &self,
        objective: &str,
        token_budget: Option<u64>,
        replace_unfinished: bool,
    ) -> Result<Goal> {
        GoalStore::new(self.goal_file()).replace(objective, token_budget, replace_unfinished)
    }

    /// 修改当前会话目标文本。
    ///
    /// 参数:
    /// - `objective`: 新目标文本
    ///
    /// 返回:
    /// - 更新后的目标
    pub(crate) fn edit_goal(&self, objective: &str) -> Result<Goal> {
        GoalStore::new(self.goal_file()).edit(objective)
    }

    /// 原位更新当前会话目标并保留累计用量。
    ///
    /// 参数:
    /// - `objective`: 可选新目标文本
    /// - `token_budget`: 可选预算更新
    /// - `status`: 可选状态更新
    ///
    /// 返回:
    /// - 更新后的目标
    pub(crate) fn update_goal(
        &self,
        objective: Option<&str>,
        token_budget: Option<Option<u64>>,
        status: Option<GoalStatus>,
    ) -> Result<Goal> {
        GoalStore::new(self.goal_file()).update_settings(objective, token_budget, status)
    }

    /// 更新当前会话目标状态。
    ///
    /// 参数:
    /// - `status`: 新状态
    ///
    /// 返回:
    /// - 更新后的目标
    pub(crate) fn set_goal_status(&self, status: GoalStatus) -> Result<Goal> {
        GoalStore::new(self.goal_file()).set_status(status)
    }

    /// 累加当前目标的一轮使用量。
    ///
    /// 参数:
    /// - `expected_id`: 本轮开始时的目标 ID
    /// - `tokens`: 本轮 Token 使用量
    /// - `elapsed_seconds`: 本轮耗时秒数
    ///
    /// 返回:
    /// - ID 匹配时返回更新后的目标
    pub(crate) fn account_goal_progress(
        &self,
        expected_id: &str,
        tokens: u64,
        elapsed_seconds: u64,
    ) -> Result<Option<Goal>> {
        GoalStore::new(self.goal_file()).account(expected_id, tokens, elapsed_seconds)
    }

    /// 清除当前会话目标。
    ///
    /// 返回:
    /// - 是否清除了已有目标
    pub(crate) fn clear_goal(&self) -> Result<bool> {
        GoalStore::new(self.goal_file()).clear()
    }

    /// 追加目标进度说明。
    ///
    /// 参数:
    /// - `message`: 进度摘要
    ///
    /// 返回:
    /// - 更新后的目标
    pub(crate) fn append_goal_progress(&self, message: &str) -> Result<Goal> {
        GoalStore::new(self.goal_file()).append_progress(message)
    }
}
