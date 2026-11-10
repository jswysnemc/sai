use super::*;

impl GoalStore {
    /// 【目标】【用量核算】按来源累计水位写入主代理、子代理和压缩用量
    /// 参数：`expected_id` 为绑定目标，`source` 为稳定来源，`tokens` 为累计量；返回匹配目标
    pub(crate) fn account_usage(
        &self,
        expected_id: &str,
        source: &str,
        tokens: u64,
    ) -> Result<Option<Goal>> {
        let _guard = goal_lock()?;
        let Some(mut goal) = read_goal(&self.path)?.filter(|goal| goal.id == expected_id) else {
            return Ok(None);
        };
        // 1. 【目标】【计费边界】显式完成或暂停后，报告性回复不继续消耗目标预算
        if !matches!(goal.status, GoalStatus::Active | GoalStatus::BudgetLimited) {
            return Ok(Some(goal));
        }
        let previous = goal.usage_sources.entry(source.to_string()).or_default();
        let delta = tokens.saturating_sub(*previous);
        if delta == 0 {
            return Ok(Some(goal));
        }
        *previous = (*previous).max(tokens);
        goal.tokens_used = goal.tokens_used.saturating_add(delta);
        if goal.status == GoalStatus::Active
            && goal
                .token_budget
                .is_some_and(|limit| goal.tokens_used >= limit)
        {
            goal.status = GoalStatus::BudgetLimited;
        }
        goal.updated_at = Utc::now().to_rfc3339();
        write_goal(&self.path, &goal)?;
        Ok(Some(goal))
    }

    /// 【目标】【轮次结束】记录运行错误或连续空响应，不改变已经由用户更新的目标
    /// 参数：目标标识、是否自动续轮、是否有活动及错误；返回更新后的匹配目标
    pub(crate) fn finish_turn(
        &self,
        expected_id: &str,
        automatic: bool,
        activity: bool,
        error: Option<&anyhow::Error>,
    ) -> Result<Option<Goal>> {
        let _guard = goal_lock()?;
        let Some(mut goal) = read_goal(&self.path)?.filter(|goal| goal.id == expected_id) else {
            return Ok(None);
        };
        if goal.status != GoalStatus::Active {
            return Ok(Some(goal));
        }
        if let Some(error) = error {
            let message = format!("{error:#}").to_ascii_lowercase();
            goal.status = if message.contains("usage_limit")
                || message.contains("insufficient_quota")
                || message.contains("quota exceeded")
                || message.contains("insufficient balance")
            {
                GoalStatus::UsageLimited
            } else {
                GoalStatus::Blocked
            };
        } else if activity || !automatic {
            goal.consecutive_empty_turns = 0;
        } else {
            goal.consecutive_empty_turns = goal.consecutive_empty_turns.saturating_add(1);
            if goal.consecutive_empty_turns >= 3 {
                goal.status = GoalStatus::Blocked;
            }
        }
        goal.updated_at = Utc::now().to_rfc3339();
        write_goal(&self.path, &goal)?;
        Ok(Some(goal))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【目标】【计费测试】检查点重放不重复计量，子代理用量参与预算；无参数、无返回值
    #[test]
    fn replayed_usage_is_idempotent_and_descendants_share_budget() {
        let dir = tempfile::tempdir().unwrap();
        let store = GoalStore::new(dir.path().join("goal.json"));
        let goal = store.replace("finish work", Some(100), false).unwrap();
        store.account_usage(&goal.id, "turn-1", 60).unwrap();
        store.account_usage(&goal.id, "turn-1", 60).unwrap();
        store.account_usage(&goal.id, "turn-1", 40).unwrap();
        let goal = store
            .account_usage(&goal.id, "subagent-1", 40)
            .unwrap()
            .unwrap();
        assert_eq!(goal.tokens_used, 100);
        assert_eq!(goal.status, GoalStatus::BudgetLimited);
        assert_eq!(
            store.set_status(GoalStatus::Paused).unwrap().status,
            GoalStatus::BudgetLimited
        );
        assert!(store.replace("other work", None, false).is_err());
    }

    /// 【目标】【续轮测试】第三次自动空响应停止续轮，恢复后重新计数；无参数、无返回值
    #[test]
    fn empty_continuations_stop_and_resume_resets_the_audit() {
        let dir = tempfile::tempdir().unwrap();
        let store = GoalStore::new(dir.path().join("goal.json"));
        let goal = store.replace("finish work", None, false).unwrap();
        for index in 1..=3 {
            let current = store
                .finish_turn(&goal.id, true, false, None)
                .unwrap()
                .unwrap();
            assert_eq!(
                current.status,
                if index == 3 {
                    GoalStatus::Blocked
                } else {
                    GoalStatus::Active
                }
            );
        }
        store.set_status(GoalStatus::Active).unwrap();
        assert_eq!(
            store
                .finish_turn(&goal.id, true, false, None)
                .unwrap()
                .unwrap()
                .status,
            GoalStatus::Active
        );
        assert_eq!(store.get().unwrap().unwrap().objective, "finish work");
    }
}
