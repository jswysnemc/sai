use super::{Goal, GoalStore};
use crate::llm::Usage;
use anyhow::Result;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// 【目标】【运行核算】共享单轮水位，模型调用完成后立即写入目标预算
#[derive(Clone)]
pub(crate) struct GoalAccounting {
    store: GoalStore,
    source: String,
    progress: Arc<Mutex<(Option<String>, u64)>>,
    clock: Arc<Mutex<(Instant, u64)>>,
}

impl GoalAccounting {
    /// 【目标】【绑定运行】绑定本轮目标，后续新建目标可从首次请求开始计量
    /// 参数：存储、来源标识和当前目标；返回核算句柄
    pub(crate) fn new(store: GoalStore, source: String, goal_id: Option<String>) -> Result<Self> {
        let previous = store
            .get()?
            .filter(|goal| Some(&goal.id) == goal_id.as_ref())
            .and_then(|goal| goal.usage_sources.get(&source).copied())
            .unwrap_or_default();
        Ok(Self {
            store,
            source,
            progress: Arc::new(Mutex::new((goal_id, previous))),
            clock: Arc::new(Mutex::new((Instant::now(), 0))),
        })
    }

    /// 【目标】【模型用量】成功请求按供应商实际用量增加水位
    /// 参数：`usage` 为供应商用量；返回持久化结果
    pub(crate) fn record(&self, usage: Option<&Usage>) -> Result<()> {
        let mut progress = self
            .progress
            .lock()
            .map_err(|_| anyhow::anyhow!("goal accounting lock poisoned"))?;
        if progress.0.is_none() {
            progress.0 = self
                .store
                .get()?
                .filter(|goal| goal.status.is_active())
                .map(|goal| goal.id);
        }
        let Some(id) = progress.0.clone() else {
            return Ok(());
        };
        progress.1 = progress
            .1
            .saturating_add(usage.map(|usage| usage.total_tokens).unwrap_or_default());
        self.store.account_usage(&id, &self.source, progress.1)?;
        drop(progress);
        self.account_time()?;
        Ok(())
    }

    /// 【目标】【时间增量】仅主代理累计实际运行时间，子代理并行时间不重复计入
    /// 参数：无；返回时间水位写入结果
    fn account_time(&self) -> Result<()> {
        if !self.source.starts_with("turn:") {
            return Ok(());
        }
        let Some(goal) = self.current_goal()?.filter(|goal| {
            matches!(
                goal.status,
                super::GoalStatus::Active | super::GoalStatus::BudgetLimited
            )
        }) else {
            return Ok(());
        };
        let mut clock = self
            .clock
            .lock()
            .map_err(|_| anyhow::anyhow!("goal clock lock poisoned"))?;
        let elapsed = clock.0.elapsed().as_secs();
        let delta = elapsed.saturating_sub(clock.1);
        if delta > 0 {
            self.store.account(&goal.id, 0, delta)?;
            clock.1 = elapsed;
        }
        Ok(())
    }

    /// 【目标】【状态读取】只读取本轮绑定目标，替换目标后不沿用旧运行权限
    /// 参数：无；返回当前匹配目标
    pub(crate) fn current_goal(&self) -> Result<Option<Goal>> {
        let id = self.goal_id();
        Ok(self
            .store
            .get()?
            .filter(|goal| Some(&goal.id) == id.as_ref()))
    }

    /// 【目标】【当前标识】返回本轮绑定目标，已替换目标不重新绑定
    /// 参数：无；返回目标标识
    fn goal_id(&self) -> Option<String> {
        self.progress.lock().ok().and_then(|value| value.0.clone())
    }
}

/// 【目标】【运行收尾】保证普通结束、错误和取消都核算耗时
pub(crate) struct GoalTurnGuard {
    accounting: GoalAccounting,
    finished: bool,
}

impl GoalTurnGuard {
    /// 【目标】【计时开始】创建一次轮次计时守卫；参数为核算句柄，返回守卫
    pub(crate) fn new(accounting: GoalAccounting) -> Self {
        Self {
            accounting,
            finished: false,
        }
    }

    /// 【目标】【状态收尾】记录自动续轮活动和错误，避免重复空转
    /// 参数：是否自动续轮、是否有实际活动、可选错误；返回持久化结果
    pub(crate) fn finish(
        &mut self,
        automatic: bool,
        activity: bool,
        error: Option<&anyhow::Error>,
    ) -> Result<()> {
        self.account_time()?;
        if let Some(id) = self.accounting.goal_id() {
            self.accounting
                .store
                .finish_turn(&id, automatic, activity, error)?;
        }
        Ok(())
    }

    /// 【目标】【时间核算】仅累计一次本轮耗时；无参数，返回存储结果
    fn account_time(&mut self) -> Result<()> {
        if self.finished {
            return Ok(());
        }
        self.accounting.account_time()?;
        self.finished = true;
        Ok(())
    }
}

impl Drop for GoalTurnGuard {
    /// 【目标】【取消核算】丢弃运行时保留已用时间，不自行暂停目标；无参数、无返回值
    fn drop(&mut self) {
        if let Err(error) = self.account_time() {
            eprintln!("【目标】【取消核算】{error:#}");
        }
    }
}
