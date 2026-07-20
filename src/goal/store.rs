use super::{Goal, GoalStatus, GoalUpdateEntry};
use anyhow::{bail, Context, Result};
use chrono::Utc;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

static GOAL_FILE_LOCK: Mutex<()> = Mutex::new(());
const MAX_GOAL_OBJECTIVE_CHARS: usize = 32_000;

/// 会话目标 JSON 存储。
#[derive(Clone, Debug)]
pub(crate) struct GoalStore {
    path: PathBuf,
}

impl GoalStore {
    /// 创建目标存储。
    ///
    /// 参数:
    /// - `path`: 会话目标文件路径
    ///
    /// 返回:
    /// - 目标存储
    pub(crate) fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// 读取当前目标。
    ///
    /// 返回:
    /// - 当前目标；文件不存在时返回空
    pub(crate) fn get(&self) -> Result<Option<Goal>> {
        let _guard = goal_lock()?;
        read_goal(&self.path)
    }

    /// 创建或替换当前目标，并重置使用量。
    ///
    /// 参数:
    /// - `objective`: 目标文本
    /// - `token_budget`: 可选 Token 预算
    /// - `replace_unfinished`: 是否允许替换未结束目标
    ///
    /// 返回:
    /// - 新目标
    pub(crate) fn replace(
        &self,
        objective: &str,
        token_budget: Option<u64>,
        replace_unfinished: bool,
    ) -> Result<Goal> {
        let objective = validate_objective(objective)?;
        validate_budget(token_budget)?;
        let _guard = goal_lock()?;
        if !replace_unfinished
            && read_goal(&self.path)?.is_some_and(|goal| !goal.status.is_terminal())
        {
            bail!("an unfinished goal already exists; update or clear it before creating another")
        }
        let now = Utc::now().to_rfc3339();
        let goal = Goal {
            id: format!("goal_{}", uuid::Uuid::new_v4().simple()),
            objective: objective.clone(),
            status: GoalStatus::Active,
            token_budget,
            tokens_used: 0,
            time_used_seconds: 0,
            created_at: now.clone(),
            updated_at: now.clone(),
            updates: vec![GoalUpdateEntry {
                at: now,
                kind: "status".to_string(),
                message: format!("Goal created: {objective}"),
                status: Some(GoalStatus::Active.as_str().to_string()),
                tokens_used: Some(0),
            }],
        };
        write_goal(&self.path, &goal)?;
        Ok(goal)
    }

    /// 修改当前目标文本并保留累计使用量。
    ///
    /// 参数:
    /// - `objective`: 新目标文本
    ///
    /// 返回:
    /// - 更新后的目标
    pub(crate) fn edit(&self, objective: &str) -> Result<Goal> {
        self.update_settings(Some(objective), None, Some(GoalStatus::Active))
    }

    /// 原位更新目标文本、预算和状态，并保留累计使用量。
    ///
    /// 参数:
    /// - `objective`: 可选新目标文本
    /// - `token_budget`: 可选预算更新；外层为空表示保持，内层为空表示取消预算
    /// - `status`: 可选新状态
    ///
    /// 返回:
    /// - 更新后的目标
    pub(crate) fn update_settings(
        &self,
        objective: Option<&str>,
        token_budget: Option<Option<u64>>,
        status: Option<GoalStatus>,
    ) -> Result<Goal> {
        let objective = objective.map(validate_objective).transpose()?;
        if let Some(token_budget) = token_budget {
            validate_budget(token_budget)?;
        }
        self.update(|goal| {
            if let Some(objective) = objective {
                if goal.objective != objective {
                    push_update(
                        goal,
                        "status",
                        "Objective updated",
                        Utc::now().to_rfc3339(),
                    );
                }
                goal.objective = objective;
            }
            if let Some(token_budget) = token_budget {
                goal.token_budget = token_budget;
            }
            if let Some(status) = status {
                let next = if status == GoalStatus::Active
                    && goal
                        .token_budget
                        .is_some_and(|budget| goal.tokens_used >= budget)
                {
                    GoalStatus::BudgetLimited
                } else {
                    status
                };
                if goal.status != next {
                    push_update(
                        goal,
                        "status",
                        &format!("Status -> {}", next.as_str()),
                        Utc::now().to_rfc3339(),
                    );
                }
                goal.status = next;
            }
            if goal.status == GoalStatus::Active
                && goal
                    .token_budget
                    .is_some_and(|budget| goal.tokens_used >= budget)
            {
                goal.status = GoalStatus::BudgetLimited;
            }
            Ok(())
        })
    }

    /// 更新当前目标状态。
    ///
    /// 参数:
    /// - `status`: 新状态
    ///
    /// 返回:
    /// - 更新后的目标
    /// 追加一条人类可读的目标进度说明。
    ///
    /// 参数:
    /// - `message`: 进度摘要
    ///
    /// 返回:
    /// - 更新后的目标
    pub(crate) fn append_progress(&self, message: &str) -> Result<Goal> {
        let message = message.trim();
        if message.is_empty() {
            bail!("progress message is empty");
        }
        self.update(|goal| {
            push_update(
                goal,
                "progress",
                message,
                Utc::now().to_rfc3339(),
            );
            Ok(())
        })
    }

        pub(crate) fn set_status(&self, status: GoalStatus) -> Result<Goal> {
        self.update(|goal| {
            let next = if status == GoalStatus::Active
                && goal
                    .token_budget
                    .is_some_and(|budget| goal.tokens_used >= budget)
            {
                GoalStatus::BudgetLimited
            } else {
                status
            };
            if goal.status != next {
                push_update(
                    goal,
                    "status",
                    &format!("Status -> {}", next.as_str()),
                    Utc::now().to_rfc3339(),
                );
            }
            goal.status = next;
            Ok(())
        })
    }

    /// 累加一轮目标使用量。
    ///
    /// 参数:
    /// - `expected_id`: 本轮开始时绑定的目标 ID
    /// - `tokens`: 本轮 Token 使用量
    /// - `elapsed_seconds`: 本轮耗时秒数
    ///
    /// 返回:
    /// - 目标仍存在且 ID 匹配时返回更新结果
    pub(crate) fn account(
        &self,
        expected_id: &str,
        tokens: u64,
        elapsed_seconds: u64,
    ) -> Result<Option<Goal>> {
        let _guard = goal_lock()?;
        let Some(mut goal) = read_goal(&self.path)? else {
            return Ok(None);
        };
        if goal.id != expected_id {
            return Ok(None);
        }
        goal.tokens_used = goal.tokens_used.saturating_add(tokens);
        goal.time_used_seconds = goal.time_used_seconds.saturating_add(elapsed_seconds);
        if goal.status == GoalStatus::Active
            && goal
                .token_budget
                .is_some_and(|budget| goal.tokens_used >= budget)
        {
            goal.status = GoalStatus::BudgetLimited;
        }
        let now = Utc::now().to_rfc3339();
        goal.updated_at = now.clone();
        push_update(
            &mut goal,
            "account",
            &format!("Turn usage +{tokens} tokens · +{elapsed_seconds}s"),
            now,
        );
        write_goal(&self.path, &goal)?;
        Ok(Some(goal))
    }

    /// 删除当前目标。
    ///
    /// 返回:
    /// - 是否删除了目标文件
    pub(crate) fn clear(&self) -> Result<bool> {
        let _guard = goal_lock()?;
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    /// 在锁内更新现有目标。
    ///
    /// 参数:
    /// - `mutate`: 目标修改函数
    ///
    /// 返回:
    /// - 更新后的目标
    fn update(&self, mutate: impl FnOnce(&mut Goal) -> Result<()>) -> Result<Goal> {
        let _guard = goal_lock()?;
        let mut goal = read_goal(&self.path)?.context("no goal exists for this session")?;
        mutate(&mut goal)?;
        goal.updated_at = Utc::now().to_rfc3339();
        write_goal(&self.path, &goal)?;
        Ok(goal)
    }
}

/// 获取目标文件全局短临界区锁。
///
/// 返回:
/// - 目标文件锁守卫
fn goal_lock() -> Result<std::sync::MutexGuard<'static, ()>> {
    GOAL_FILE_LOCK
        .lock()
        .map_err(|_| anyhow::anyhow!("goal file lock is poisoned"))
}

/// 读取未加锁目标文件。
///
/// 参数:
/// - `path`: 目标文件路径
///
/// 返回:
/// - 当前目标

/// 向目标写入一条更新记录，并限制历史长度。
///
/// 参数:
/// - `goal`: 待修改目标
/// - `kind`: 更新类型
/// - `message`: 摘要
/// - `at`: 时间戳
///
/// 返回:
/// - 无
fn push_update(goal: &mut Goal, kind: &str, message: &str, at: String) {
    goal.updates.push(GoalUpdateEntry {
        at,
        kind: kind.to_string(),
        message: message.to_string(),
        status: Some(goal.status.as_str().to_string()),
        tokens_used: Some(goal.tokens_used),
    });
    const MAX_UPDATES: usize = 200;
    if goal.updates.len() > MAX_UPDATES {
        let drop = goal.updates.len() - MAX_UPDATES;
        goal.updates.drain(0..drop);
    }
}

fn read_goal(path: &Path) -> Result<Option<Goal>> {
    let raw = match std::fs::read(path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    serde_json::from_slice(&raw)
        .context("invalid goal state")
        .map(Some)
}

/// 原子写入目标文件。
///
/// 参数:
/// - `path`: 目标文件路径
/// - `goal`: 待保存目标
///
/// 返回:
/// - 写入是否成功
fn write_goal(path: &Path, goal: &Goal) -> Result<()> {
    let parent = path
        .parent()
        .context("goal state path has no parent directory")?;
    std::fs::create_dir_all(parent)?;
    let mut content = serde_json::to_vec_pretty(goal)?;
    content.push(b'\n');
    let temp = tempfile::NamedTempFile::new_in(parent)?;
    std::fs::write(temp.path(), content)?;
    temp.persist(path)?;
    Ok(())
}

/// 校验并归一化目标文本。
///
/// 参数:
/// - `objective`: 原始目标文本
///
/// 返回:
/// - 归一化目标文本
fn validate_objective(objective: &str) -> Result<String> {
    let objective = objective.trim();
    if objective.is_empty() {
        bail!("goal objective cannot be empty")
    }
    if objective.chars().count() > MAX_GOAL_OBJECTIVE_CHARS {
        bail!("goal objective is too long")
    }
    Ok(objective.to_string())
}

/// 校验可选 Token 预算。
///
/// 参数:
/// - `token_budget`: 可选预算
///
/// 返回:
/// - 预算是否合法
fn validate_budget(token_budget: Option<u64>) -> Result<()> {
    if token_budget == Some(0) {
        bail!("goal token budget must be greater than zero")
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accounts_usage_and_stops_at_budget() {
        let temp = tempfile::tempdir().unwrap();
        let store = GoalStore::new(temp.path().join("goal.json"));
        let goal = store.replace("finish feature", Some(10), true).unwrap();

        let updated = store.account(&goal.id, 10, 3).unwrap().unwrap();

        assert_eq!(updated.tokens_used, 10);
        assert_eq!(updated.time_used_seconds, 3);
        assert_eq!(updated.status, GoalStatus::BudgetLimited);
    }

    #[test]
    fn refuses_to_replace_unfinished_goal_without_override() {
        let temp = tempfile::tempdir().unwrap();
        let store = GoalStore::new(temp.path().join("goal.json"));
        store.replace("first", None, true).unwrap();

        let error = store.replace("second", None, false).unwrap_err();

        assert!(error.to_string().contains("unfinished goal"));
    }

    #[test]
    fn updates_goal_settings_without_resetting_usage() {
        let temp = tempfile::tempdir().unwrap();
        let store = GoalStore::new(temp.path().join("goal.json"));
        let goal = store.replace("first", Some(100), true).unwrap();
        store.account(&goal.id, 40, 3).unwrap();

        let updated = store
            .update_settings(Some("second"), Some(Some(200)), Some(GoalStatus::Active))
            .unwrap();

        assert_eq!(updated.objective, "second");
        assert_eq!(updated.token_budget, Some(200));
        assert_eq!(updated.tokens_used, 40);
        assert_eq!(updated.time_used_seconds, 3);
        assert_eq!(updated.id, goal.id);
    }
}
