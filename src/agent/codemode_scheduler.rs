use super::{Agent, AgentEvent};
use crate::llm::ChatMessage;
use crate::tools::codemode::{self, process::Worker};
use anyhow::{bail, Result};
use sai_codemode::output::Output;
use sai_codemode::protocol::{ExecutionRequest, ToolReply, WorkerEvent, MAX_CALLS};
use std::collections::VecDeque;
use tokio::sync::mpsc;
use tokio::task::JoinSet;

impl Agent {
    /// 【CodeMode】【宿主调度】运行 request，messages 提供审核上下文，collected/on_event 接收输出。
    /// 返回脚本错误与状态增量；只读工具最多四路并发，写入和交互与其它调用串行。
    pub(super) async fn run_codemode_worker(
        &mut self,
        request: ExecutionRequest,
        messages: &[ChatMessage],
        collected: &mut Output,
        on_event: &mut impl FnMut(AgentEvent) -> Result<()>,
    ) -> Result<(Option<String>, sai_codemode::store::Store)> {
        let mut worker = Worker::start(&std::env::current_exe()?, &request).await?;
        let mut pending = VecDeque::new();
        let mut jobs = JoinSet::new();
        let (progress, mut progress_rx) = mpsc::channel(128);
        let mut last_id = 0;
        let mut questions = 0;
        loop {
            // 1. 【CodeMode】【并发边界】写入等待已有只读任务完成，避免脚本内读写竞争
            if let Some(event) = pending.front() {
                if jobs.len() < 4 {
                    if let Some((id, call)) = self.codemode_readonly_call(event) {
                        pending.pop_front();
                        on_event(AgentEvent::ToolProgress {
                            name: codemode::NAME.into(),
                            message: format!("{} running", call.function.name),
                        })?;
                        self.spawn_codemode_readonly(id, call, &mut jobs, progress.clone());
                        continue;
                    }
                }
                if jobs.is_empty() {
                    let event = pending.pop_front().unwrap();
                    let WorkerEvent::Call { id, .. } = &event else {
                        unreachable!()
                    };
                    let id = *id;
                    let result = {
                        let call =
                            self.execute_codemode_call(event, messages, &mut questions, on_event);
                        tokio::pin!(call);
                        loop {
                            // 2. 【CodeMode】【完成优先】工具等待期间继续读输出；脚本结束立即取消未等待调用
                            tokio::select! {
                                result = &mut call => break result,
                                frame = worker.next() => {
                                    match frame? {
                                        event @ WorkerEvent::Call { .. } => enqueue(&mut pending, &mut last_id, event)?,
                                        WorkerEvent::Output { kind, value } => collected.push(kind, value)?,
                                        WorkerEvent::Done { error, writes } => {
                                            worker.stop().await?;
                                            return Ok((error, writes));
                                        }
                                    }
                                }
                            }
                        }
                    };
                    worker.reply(&reply(id, result)).await?;
                    continue;
                }
            }
            tokio::select! {
                frame = worker.next() => {
                    match frame? {
                        event @ WorkerEvent::Call { .. } => enqueue(&mut pending, &mut last_id, event)?,
                        WorkerEvent::Output { kind, value } => collected.push(kind, value)?,
                        WorkerEvent::Done { error, writes } => {
                            worker.stop().await?;
                            return Ok((error, writes));
                        }
                    }
                }
                Some(completion) = jobs.join_next(), if !jobs.is_empty() => {
                    let (id, call, result) = completion?;
                    let result = result.and_then(|output| {
                        super::edit_guard::record_successful_reads(self, &call, &output.content)?;
                        codemode::dispatch::result_value(output)
                    });
                    on_event(AgentEvent::ToolProgress { name: codemode::NAME.into(), message: format!("{} {}", call.function.name, if result.is_ok() {"completed"} else {"failed"}) })?;
                    worker.reply(&reply(id, result)).await?;
                }
                Some(event) = progress_rx.recv() => on_event(event)?,
            }
        }
    }
}

/// 【CodeMode】【请求排队】把 event 加入 pending 并更新 last_id，返回序号与容量校验结果。
fn enqueue(
    pending: &mut VecDeque<WorkerEvent>,
    last_id: &mut u64,
    event: WorkerEvent,
) -> Result<()> {
    let WorkerEvent::Call { id, .. } = &event else {
        bail!("expected tool call");
    };
    if *id <= *last_id || *id > MAX_CALLS as u64 || pending.len() >= 128 {
        bail!("invalid codemode call sequence or queue limit exceeded");
    }
    *last_id = *id;
    pending.push_back(event);
    Ok(())
}

/// 【CodeMode】【工具回复】把 id/result 转为宿主回复，返回成功值或可捕获错误。
fn reply(id: u64, result: Result<serde_json::Value>) -> ToolReply {
    match result {
        Ok(value) => ToolReply {
            id,
            value,
            error: None,
        },
        Err(error) => ToolReply {
            id,
            value: serde_json::Value::Null,
            error: Some(error.to_string()),
        },
    }
}
