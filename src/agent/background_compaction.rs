use super::{Agent, AgentEvent};
use crate::llm::{ChatMessage, ChatResult};
use crate::state::request_projection::project_provider_turn_from_messages;
use crate::state::{CompactionApplyOutcome, CompactionRequest};
use anyhow::Result;
use sha2::{Digest, Sha256};
use tokio::task::JoinHandle;

/// 【上下文】【后台摘要】持有精确来源和可取消任务，执行器释放时不遗留模型请求。
pub(super) struct BackgroundCompaction {
    request: CompactionRequest,
    source: String,
    handle: Option<JoinHandle<Result<ChatResult>>>,
}

impl Drop for BackgroundCompaction {
    /// 【上下文】【取消预计算】销毁或替换预计算时取消请求；无参数、无返回值。
    fn drop(&mut self) {
        if let Some(handle) = &self.handle {
            handle.abort();
        }
    }
}

impl Agent {
    /// 【上下文】【摘要指纹】重新读取来源轮次与工具记录，验证历史、摘要和模型路由未改变。
    /// 参数：压缩范围；返回完整输入及不可逆指纹，来源失效时为空。
    fn background_compaction_source(
        &self,
        request: &CompactionRequest,
    ) -> Result<Option<(Vec<ChatMessage>, String)>> {
        let previous = self
            .state
            .load_authoritative_compaction_summary()?
            .map(|summary| summary.summary);
        if previous != request.previous_summary {
            return Ok(None);
        }
        let turns = self.state.load_turns()?;
        let mut refreshed = request.clone();
        refreshed.compact_turns.clear();
        for id in &request.compact_turn_ids {
            let Some(turn) = turns.iter().find(|turn| {
                &turn.turn_id == id && turn.status != crate::state::TurnStatus::Running
            }) else {
                return Ok(None);
            };
            refreshed.compact_turns.push(turn.clone());
        }
        if let Some(running) = &request.running_turn {
            if !turns.iter().any(|turn| {
                turn.turn_id == running.turn_id && turn.status == crate::state::TurnStatus::Running
            }) {
                return Ok(None);
            }
            if self
                .state
                .running_turn_compaction_boundary()?
                .is_some_and(|(id, calls)| {
                    id == running.turn_id && calls >= running.compacted_calls
                })
            {
                return Ok(None);
            }
        }
        let prompt = self.state.build_compaction_summary_prompt(
            &refreshed,
            self.context_char_budget,
            &self.config.prompt.templates.compaction,
        )?;
        let runtime = self.config.compaction_runtime_config()?;
        let mut hash = Sha256::new();
        hash.update(serde_json::to_vec(runtime.provider(None)?)?);
        hash.update(self.context_char_budget.to_le_bytes());
        hash.update(prompt.system.as_bytes());
        hash.update([0]);
        hash.update(prompt.user.as_bytes());
        hash.update(
            format!(
                "{:?}:{:?}",
                refreshed.compact_turn_ids, refreshed.running_turn
            )
            .as_bytes(),
        );
        let source = hex::encode(hash.finalize());
        Ok(Some((
            vec![
                ChatMessage::system(prompt.system),
                ChatMessage::plain("user", prompt.user),
            ],
            source,
        )))
    }

    /// 【上下文】【后台预压缩】阈值前生成摘要，工具间隙验证来源后应用；新指导与最近工具仍由原投影保留。
    /// 参数：当前轮次、实际消息、占用和事件回调；返回是否已经应用摘要。
    pub(super) async fn maintain_background_compaction(
        &mut self,
        turn_id: &str,
        messages: &[ChatMessage],
        occupancy: usize,
        on_event: &mut impl FnMut(AgentEvent) -> Result<()>,
    ) -> Result<bool> {
        let trigger = self
            .compaction_budget_policy()
            .trigger_chars(self.context_char_budget);
        if trigger == 0 {
            self.background_compaction = None;
            return Ok(false);
        }
        let should_wait = occupancy >= trigger;
        if let Some(pending) = self.background_compaction.as_ref() {
            let valid = self
                .background_compaction_source(&pending.request)?
                .is_some_and(|(_, source)| source == pending.source);
            if !valid {
                self.background_compaction = None;
            }
        }
        if self.background_compaction.as_ref().is_some_and(|pending| {
            should_wait || pending.handle.as_ref().is_some_and(JoinHandle::is_finished)
        }) {
            let mut pending = self.background_compaction.take().unwrap();
            let result = pending.handle.as_mut().unwrap().await;
            match result {
                Ok(Ok(result)) => {
                    // 1. 【上下文】【提交校验】等待期间也可能收到用户编辑，提交前再读取来源
                    if !self
                        .background_compaction_source(&pending.request)?
                        .is_some_and(|(_, source)| source == pending.source)
                    {
                        return Ok(false);
                    }
                    crate::state::validate_summary(
                        &result.content,
                        crate::state::summary_char_limit(self.context_char_budget),
                    )?;
                    let summary = self.finalize_summary(&pending.request, &result.content);
                    let mut projection =
                        project_provider_turn_from_messages(messages, 0, self.context_char_budget);
                    projection.context_blocks = self.context_blocks_enabled();
                    on_event(AgentEvent::CompactionStarted {
                        turn_count: pending.request.turn_count(),
                        model: self.compaction_model_label.clone(),
                    })?;
                    let applied = self.state.apply_compaction_with_budget_guard(
                        &pending.request,
                        &summary,
                        &projection,
                        Some(turn_id),
                    )? == CompactionApplyOutcome::Applied;
                    if applied {
                        self.tool_visibility.clear_loaded_skills();
                        self.state.clear_loaded_skills()?;
                        self.record_evicted_turns(&pending.request);
                    }
                    on_event(AgentEvent::CompactionFinished {
                        applied,
                        summary: applied.then_some(summary),
                        error: None,
                    })?;
                    return Ok(applied);
                }
                Ok(Err(error)) => {
                    eprintln!("【上下文】【后台预压缩】摘要失败，将按需重试: {error:#}")
                }
                Err(error) => eprintln!("【上下文】【后台预压缩】任务中断，将按需重试: {error}"),
            }
        }
        if self.background_compaction.is_none()
            && !should_wait
            && occupancy >= trigger.saturating_mul(85) / 100
        {
            let projection =
                project_provider_turn_from_messages(messages, 0, self.context_char_budget);
            let Some(request) = self
                .state
                .select_compaction_for_projection(&projection, true)?
            else {
                return Ok(false);
            };
            let Some((messages, source)) = self.background_compaction_source(&request)? else {
                return Ok(false);
            };
            let client = self.compaction_client.clone();
            let state = self.state.clone();
            let retry = self.config.retry.clone();
            let accounting = self.goal_accounting.clone();
            let paths = self.paths.clone();
            let cancel = self.cancel_requested.clone();
            let limit = crate::state::summary_char_limit(self.context_char_budget);
            let handle = tokio::spawn(async move {
                let started = std::time::Instant::now();
                let request =
                    client.chat_stream_events_retry(messages, Vec::new(), &retry, |_| Ok(()));
                tokio::pin!(request);
                let mut tick = tokio::time::interval(std::time::Duration::from_millis(100));
                let result = loop {
                    tokio::select! {
                        result = &mut request => break result?,
                        _ = tick.tick() => {
                            anyhow::ensure!(!cancel.load(std::sync::atomic::Ordering::Acquire), "background compaction cancelled");
                            if let Some(accounting) = &accounting {
                                anyhow::ensure!(accounting.current_goal()?.is_none_or(|goal| goal.status.is_active()), "background compaction stopped with goal");
                            }
                        }
                    }
                };
                // 2. 【上下文】【预计算用量】即使摘要后来失效，已消耗用量仍立即核算
                if let Some(usage) = &result.usage {
                    if let Some(accounting) = accounting {
                        accounting.record(Some(usage))?;
                    }
                    state.add_auxiliary_usage(usage)?;
                    let _ = crate::usage_history::record_model_call(
                        &paths,
                        crate::usage_history::UsageRecordInput {
                            provider_id: client.provider_id(),
                            provider_name: client.provider_name(),
                            model: client.model(),
                            source: "compaction",
                            operation: "background",
                            status: "success",
                            usage: Some(usage),
                            usage_source: "provider_reported",
                            started_at: chrono::Utc::now().timestamp(),
                            duration_ms: started.elapsed().as_millis() as u64,
                            session_id: Some(state.session_id()),
                            error_kind: None,
                        },
                    );
                }
                anyhow::ensure!(
                    result.tool_calls.is_empty(),
                    "background compaction requested tools"
                );
                crate::state::validate_summary(&result.content, limit)?;
                Ok(result)
            });
            self.background_compaction = Some(BackgroundCompaction {
                request,
                source,
                handle: Some(handle),
            });
        }
        Ok(false)
    }
}

#[cfg(test)]
#[path = "background_compaction_tests.rs"]
mod tests;
