use crate::agent::Agent;
use crate::cli::repl_turn_task::ReplTurnTask;
use std::ops::{Deref, DerefMut};

/// 会话执行槽：空闲时持有 Agent，运行时只由后台轮次独占 Agent。
pub(in crate::cli) struct ReplAgentSlot {
    pub(in crate::cli) agent: Option<Agent>,
    pub(in crate::cli) task: Option<ReplTurnTask>,
}

impl ReplAgentSlot {
    /// 包装已有 Agent；参数为会话实例，返回未运行的槽，不重复调用插件初始化。
    pub(in crate::cli) fn new(agent: Agent) -> Self {
        Self {
            agent: Some(agent),
            task: None,
        }
    }

    /// 查询是否仍有待接管的轮次；参数无，返回运行或尚未收尾标志。
    pub(in crate::cli) fn is_running(&self) -> bool {
        self.task.is_some()
    }
}

impl Deref for ReplAgentSlot {
    type Target = Agent;

    /// 借用空闲 Agent；参数无，返回独占轮次之外的只读引用。
    fn deref(&self) -> &Agent {
        self.agent
            .as_ref()
            .expect("running Agent must be reattached before commands")
    }
}

impl DerefMut for ReplAgentSlot {
    /// 借用空闲 Agent；参数无，返回独占轮次之外的可变引用。
    fn deref_mut(&mut self) -> &mut Agent {
        self.agent
            .as_mut()
            .expect("running Agent must be reattached before commands")
    }
}
