use super::*;
use crate::agent::{AgentMode, LiveAgentMode};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

struct ModeQueue {
    expected: AgentMode,
    live: Arc<LiveAgentMode>,
    change_once: AtomicBool,
}

#[async_trait::async_trait]
impl InterMessageSource for ModeQueue {
    /// 【消息队列】【测试入口】无参数；禁止测试通过不读取实时模式的路径获取消息。
    async fn peek(&self) -> Result<Option<InterMessage>> {
        panic!("the Agent must supply its live mode");
    }

    /// 【消息队列】【测试筛选】参数 mode 为实时模式；模拟等待期间切换权限并返回匹配消息。
    async fn peek_for_mode(&self, mode: AgentMode) -> Result<Option<InterMessage>> {
        if self.change_once.swap(false, Ordering::SeqCst) {
            self.live.set(AgentMode::Audited.with_codemode(true));
        }
        Ok(
            (mode == self.expected)
                .then(|| InterMessage::queued_user("queued", "guidance", vec![])),
        )
    }

    /// 【消息队列】【测试确认】参数为消息标识；选择阶段不得确认，返回无。
    async fn acknowledge(&self, _message_id: &str) -> Result<()> {
        panic!("selection must not acknowledge messages");
    }
}

/// 【消息队列】【动态权限】无参数；验证读取与等待后的模式变化都不会把计划消息交给可写任务。
#[tokio::test]
async fn gap_delivery_rechecks_live_mode_after_reading_the_source() {
    let root = tempfile::tempdir().unwrap();
    let paths = crate::paths::SaiPaths::for_tests(root.path());
    let mut config = crate::config::AppConfig::default();
    config.skills.enabled = false;
    config.load_instruction_files = false;
    let state = crate::state::StateStore::new(&paths).unwrap();
    let client = crate::llm::OpenAiCompatibleClient::from_config(&config, &paths).unwrap();
    let plan = AgentMode::Plan.with_codemode(true);
    let agent = Agent::new(
        config,
        &paths,
        state,
        client,
        crate::tools::ToolRegistry::new(),
        plan,
    )
    .unwrap();
    let source = ModeQueue {
        expected: plan,
        live: agent.live_mode_handle(),
        change_once: AtomicBool::new(true),
    };
    assert!(agent
        .next_gap_message(Some(&source), false, false, &mut |_| Ok(()))
        .await
        .unwrap()
        .is_none());
    agent.apply_live_mode(plan);
    assert!(agent
        .next_gap_message(Some(&source), false, false, &mut |_| Ok(()))
        .await
        .unwrap()
        .is_some());
    agent.apply_live_mode(AgentMode::Yolo.with_codemode(true));
    assert!(agent
        .next_gap_message(Some(&source), false, false, &mut |_| Ok(()))
        .await
        .unwrap()
        .is_none());
}
