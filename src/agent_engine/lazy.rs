use super::{EventSender, ExternalTurnEngine, TurnRequest};
use crate::config::AgentEngineConfig;
use crate::llm::ChatResult;
use anyhow::{Context, Result};
use std::time::Duration;

/// 延迟启动的 ACP 内核。
///
/// `Agent` 会在很多不产生对话的场景被构造——配置页预览、上下文估算、工具清单统计。
/// 若在构造时就拉起外部进程，这些场景都会白白启动一个 agent 并可能触发登录流程，
/// 因此把进程启动与握手推迟到第一轮真正的对话。
pub(crate) struct LazyAcpEngine {
    name: String,
    config: AgentEngineConfig,
    governance: crate::acp::AcpGovernance,
    inner: Option<crate::acp::AcpEngine>,
}

impl LazyAcpEngine {
    /// 创建尚未启动的内核。
    ///
    /// 参数:
    /// - `name`: 内核标识
    /// - `config`: 内核配置
    /// - `governance`: 治理句柄，外部内核的落盘与执行都要过它
    ///
    /// 返回:
    /// - 未连接的内核
    pub(crate) fn new(
        name: String,
        config: AgentEngineConfig,
        governance: crate::acp::AcpGovernance,
    ) -> Self {
        Self {
            name,
            config,
            governance,
            inner: None,
        }
    }

    /// 确保外部进程已启动并完成握手。
    ///
    /// 参数:
    /// - `cwd`: 会话工作目录
    ///
    /// 返回:
    /// - 已连接的内核
    async fn ensure_connected(
        &mut self,
        cwd: &std::path::Path,
    ) -> Result<&mut crate::acp::AcpEngine> {
        if self.inner.is_none() {
            let (program, args) = self
                .config
                .resolved_command()
                .context("agent engine has no launch command")?;
            let engine = crate::acp::AcpEngine::connect(
                &self.name,
                &program,
                &args,
                &self.config.acp.env,
                cwd,
                Duration::from_secs(self.config.acp.startup_timeout_seconds),
                self.governance.clone(),
            )
            .await?;
            self.inner = Some(engine);
        }
        Ok(self
            .inner
            .as_mut()
            .expect("engine was just connected above"))
    }
}

#[async_trait::async_trait]
impl ExternalTurnEngine for LazyAcpEngine {
    fn name(&self) -> &str {
        &self.name
    }

    async fn run_turn(&mut self, request: TurnRequest, events: EventSender) -> Result<ChatResult> {
        let cwd = request.cwd.clone();
        let first_connect = self.inner.is_none();
        let engine = self.ensure_connected(&cwd).await?;
        // 首次连上后播报 agentInfo：它来自握手响应，只有真正拉起外部进程才拿得到，
        // 因此这是「本轮由谁执行」的运行时证据，而不是配置读数的复述
        if first_connect {
            if let Some((name, version)) = engine.agent_info() {
                let _ = events.send(crate::agent::AgentEvent::EngineReady {
                    engine: name,
                    version,
                });
            }
        }
        engine.run_turn(request, events).await
    }

    /// 延迟连接外部内核并压缩当前会话。
    ///
    /// 参数:
    /// - `cwd`: 当前会话工作目录
    /// - `events`: 压缩生命周期事件发送端
    ///
    /// 返回:
    /// - 外部内核报告的压缩结果
    async fn compact(
        &mut self,
        cwd: std::path::PathBuf,
        events: EventSender,
    ) -> Result<crate::agent::CompactionRunOutcome> {
        self.ensure_connected(&cwd)
            .await?
            .compact(cwd, events)
            .await
    }

    async fn shutdown(&mut self) -> Result<()> {
        match self.inner.as_mut() {
            Some(engine) => engine.shutdown().await,
            // 从未启动过进程，无需回收
            None => Ok(()),
        }
    }
}
