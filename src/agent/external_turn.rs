use super::external_tool_history::ExternalToolHistory;
use super::{Agent, AgentEvent};
use crate::agent_engine::{AcpPromptContext, TurnRequest};
use crate::llm::ChatResult;
use crate::state::PendingTurnGuard;
use anyhow::Result;

impl Agent {
    /// 判断本轮是否交给外部内核执行。
    ///
    /// 返回:
    /// - 配置了可用的外部内核时为 true
    pub(super) fn uses_external_engine(&self) -> bool {
        self.external_engine.is_some()
    }

    /// 正常关闭外部 ACP 会话与子进程。
    ///
    /// 返回:
    /// - 没有外部内核时直接成功，否则返回内核关闭结果
    pub(crate) async fn shutdown_external_engine(&mut self) -> Result<()> {
        match self.external_engine.as_mut() {
            Some(engine) => engine.shutdown().await,
            None => Ok(()),
        }
    }

    /// 用外部内核执行一轮对话。
    ///
    /// 保留 Sai 的治理与持久化外壳，包括轮次记录、未完成轮守卫、工作树撤销点、
    /// 关联记忆和活动目标；对话历史仍由外部内核自己维护。
    ///
    /// 参数:
    /// - `input`: 用户输入
    /// - `image_urls`: 随本轮提交的图片
    /// - `turn_id`: 轮次标识
    /// - `on_event`: 流式事件回调
    ///
    /// 返回:
    /// - 本轮结果
    pub(super) async fn run_external_turn<F>(
        &mut self,
        input: &str,
        image_urls: Vec<String>,
        turn_id: String,
        mut on_event: F,
    ) -> Result<ChatResult>
    where
        F: FnMut(AgentEvent) -> Result<()>,
    {
        // 【Sai/ACP】【外部轮次】1. 与原生路径一致地登记轮次，历史与时间线因此不区分内核
        self.state
            .start_turn_with_images(&turn_id, input, &image_urls)?;
        // 外部轮次同样按轮记录模型，与内置内核共用时间线的模型切换分割线口径
        if let Some(model) = super::model_context::current_model_id(&self.config) {
            let _ = self.state.set_turn_model(&turn_id, &model);
        }
        // 外部内核同样累积流式增量：提前结束时已展示的正文才能随轮次落库
        let partial_content_sink = crate::state::PartialTurnSink::new();
        let guard = PendingTurnGuard::new(
            self.state.clone(),
            turn_id.clone(),
            partial_content_sink.clone(),
        )
        .with_cancel_flag(self.cancel_requested.clone());
        let cwd = crate::runtime_cwd::current_dir()?;
        let worktree_undo =
            crate::state::worktree_undo::WorktreeUndoGuard::begin(&self.state, &cwd, &turn_id)?;
        let contexts = self.external_prompt_contexts(input)?;
        let request = TurnRequest {
            input: input.to_string(),
            image_urls,
            cwd,
            contexts,
        };
        // 【Sai/ACP】【外部轮次】2. 交给外部内核跑完一轮。内核只拿到通道发送端，
        //    回调留在本函数里驱动：它借自调用方栈上的闭包，不保证能跨线程移动，
        //    而整个对话 future 在网关路径上会被 tokio::spawn
        let engine = self
            .external_engine
            .as_mut()
            .expect("external engine must exist when uses_external_engine() is true");
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let mut tool_history = ExternalToolHistory::new(self.state.clone(), turn_id.clone())?;
        let mut turn = Box::pin(engine.run_turn(request, sender));
        let result = loop {
            tokio::select! {
                // 优先派发事件，保证 UI 的更新顺序与内核产出顺序一致
                biased;
                Some(event) = receiver.recv() => {
                    tool_history.record(&event)?;
                    if let AgentEvent::Chunk(chunk) = &event {
                        partial_content_sink.append(chunk.kind, &chunk.text);
                    }
                    on_event(event)?;
                },
                outcome = &mut turn => break outcome,
            }
        };
        // 【Sai/ACP】【外部轮次】3. 内核已返回，把仍在通道里的尾部事件排空后再收尾
        while let Ok(event) = receiver.try_recv() {
            tool_history.record(&event)?;
            if let AgentEvent::Chunk(chunk) = &event {
                partial_content_sink.append(chunk.kind, &chunk.text);
            }
            on_event(event)?;
        }
        drop(turn);
        match result {
            Ok(result) => {
                // 【Sai/ACP】【外部轮次】4. 助手回复写回会话，恢复历史时与原生内核同构
                //    先落终态再收尾：worktree 清理失败不应让完整回复显示成未完成
                guard.complete(&result.content, result.reasoning.as_deref())?;
                worktree_undo.finish();
                self.spawn_session_memory_extraction();
                Ok(result)
            }
            Err(error) => {
                let _ = guard.fail(&error.to_string());
                Err(error)
            }
        }
    }

    /// 构造外部 ACP 轮次需要的动态上下文。
    ///
    /// 参数:
    /// - `input`: 当前用户输入
    ///
    /// 返回:
    /// - 可作为 ACP 嵌入资源发送的记忆与目标上下文
    fn external_prompt_contexts(&self, input: &str) -> Result<Vec<AcpPromptContext>> {
        let mut contexts = Vec::new();
        // 【Sai/ACP】【上下文注入】1. 注入记忆索引，与内置引擎走同一条渲染
        let workspace = crate::runtime_cwd::current_dir()
            .ok()
            .map(|path| path.display().to_string());
        if let Some(memory) = self.memory.recall_for_turn(input, workspace.as_deref())? {
            contexts.push(AcpPromptContext {
                uri: "sai://memory/index".to_string(),
                text: memory,
            });
        }
        // 【Sai/ACP】【上下文注入】2. 活动目标每轮重新读取，自动续轮始终获得最新预算和状态
        if let Some(goal) = self.state.goal()?.filter(|goal| goal.status.is_active()) {
            contexts.push(AcpPromptContext {
                uri: "sai://goal/active".to_string(),
                text: crate::goal::system_context(&goal),
            });
        }
        Ok(contexts)
    }

    /// 通过外部内核执行手动上下文压缩并转发事件。
    ///
    /// 参数:
    /// - `on_event`: 压缩生命周期事件回调
    ///
    /// 返回:
    /// - 外部会话压缩结果
    pub(super) async fn compact_external_conversation(
        &mut self,
        on_event: &mut impl FnMut(AgentEvent) -> Result<()>,
    ) -> Result<super::CompactionRunOutcome> {
        // 【Sai/ACP】【手动压缩】1. 启动外部压缩并建立事件通道
        let cwd = crate::runtime_cwd::current_dir()?;
        let engine = self
            .external_engine
            .as_mut()
            .expect("external engine must exist when uses_external_engine() is true");
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        let mut compact = Box::pin(engine.compact(cwd, sender));
        // 【Sai/ACP】【手动压缩】2. 并发转发压缩生命周期，直到内核返回结果
        let outcome = loop {
            tokio::select! {
                biased;
                Some(event) = receiver.recv() => on_event(event)?,
                outcome = &mut compact => break outcome,
            }
        };
        // 【Sai/ACP】【手动压缩】3. 响应可能早于尾部更新，结束前排空通道
        while let Ok(event) = receiver.try_recv() {
            on_event(event)?;
        }
        outcome
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentMode;
    use crate::config::AppConfig;
    use crate::llm::OpenAiCompatibleClient;
    use crate::paths::SaiPaths;
    use crate::state::StateStore;
    use crate::tools::ToolRegistry;

    /// 外部轮次必须同时获得关联记忆和最新活动目标。
    #[test]
    fn builds_memory_and_goal_contexts_for_external_turns() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        let config = AppConfig::default();
        let state = StateStore::new(&paths).unwrap();
        state.init_files().unwrap();
        state
            .replace_goal("Complete the Codex ACP integration", Some(10_000), false)
            .unwrap();
        let client = OpenAiCompatibleClient::from_config(&config, &paths).unwrap();
        let agent = Agent::new(
            config,
            &paths,
            state,
            client,
            ToolRegistry::new(),
            AgentMode::Yolo,
        )
        .unwrap();
        let workspace = crate::runtime_cwd::current_dir().unwrap();
        agent
            .memory
            .notes(Some(&workspace))
            .save(
                crate::memory::file_store::MemoryScope::Project,
                &crate::memory::file_store::MemoryEntry {
                    front: crate::memory::file_store::Frontmatter {
                        name: "acp-embedded-resources".to_string(),
                        description: "Codex ACP supports embedded resources".to_string(),
                        memory_type: crate::memory::file_store::MemoryType::Project,
                    },
                    body: "正文".to_string(),
                },
                "Codex ACP supports embedded resources",
            )
            .unwrap();

        let contexts = agent
            .external_prompt_contexts("Codex ACP resources")
            .unwrap();

        assert!(contexts.iter().any(|context| {
            context.uri == "sai://memory/index" && context.text.contains("embedded resources")
        }));
        assert!(contexts.iter().any(|context| {
            context.uri == "sai://goal/active"
                && context.text.contains("Complete the Codex ACP integration")
        }));
    }

    /// 创建隔离的应用路径集合。
    ///
    /// 参数:
    /// - `root`: 临时目录根路径
    ///
    /// 返回:
    /// - 测试用 Sai 路径
    fn test_paths(root: &std::path::Path) -> SaiPaths {
        SaiPaths {
            config_dir: root.join("config"),
            config_file: root.join("config/config.jsonc"),
            secrets_file: root.join("config/secrets.jsonc"),
            skills_dir: root.join("config/skills"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            state_dir: root.join("state"),
            pictures_dir: root.join("pictures"),
            fish_hook_file: root.join("fish/sai.fish"),
            bash_hook_file: root.join("shell/bash-hook.sh"),
            zsh_hook_file: root.join("shell/zsh-hook.zsh"),
            powershell_hook_file: root.join("shell/powershell-hook.ps1"),
        }
    }
}
