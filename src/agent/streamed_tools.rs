use super::{evaluate_tool_gate, tool_execution::RealToolExecution, ToolGate, ToolVisibility};
use crate::llm::ToolCall;
use crate::tools::{ToolOutput, ToolPermission, ToolRegistry};
use anyhow::Result;
use std::{collections::HashMap, path::PathBuf, sync::Mutex};
use tokio::task::JoinHandle;

/// 【工具流】【预执行任务】只缓存本响应的只读前缀，丢弃任务时同步请求取消。
struct PendingRead {
    index: usize,
    call: ToolCall,
    handle: JoinHandle<Result<ToolOutput>>,
}
impl Drop for PendingRead {
    /// 【工具流】【取消读取】无参数、无返回值；避免响应失败后后台读取继续占用资源。
    fn drop(&mut self) {
        self.handle.abort();
    }
}

/// 【工具流】【结果缓存】读操作与响应并行，写操作始终经过正常执行与持久化边界。
#[derive(Default)]
pub(super) struct StreamedTools {
    pending: Mutex<HashMap<String, PendingRead>>,
    pub(super) cleanup: std::sync::Arc<Mutex<Vec<tokio::task::AbortHandle>>>,
    next: usize,
    sealed: bool,
    pub(super) cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub(super) accounting: Option<crate::goal::GoalAccounting>,
}
/// 【工具流】【轮次清理】正常结束、错误和取消均终止本轮尚未消费的预执行。
pub(super) struct StreamedReadGuard(std::sync::Arc<Mutex<Vec<tokio::task::AbortHandle>>>);
impl Drop for StreamedReadGuard {
    /// 【工具流】【统一收尾】无参数、无返回值；完成任务的 abort 不改变已交付结果。
    fn drop(&mut self) {
        for handle in self
            .0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .drain(..)
        {
            handle.abort();
        }
    }
}

impl StreamedTools {
    /// 【工具流】【轮次守卫】共享取消记录；无参数，返回在轮次退出时清理的守卫。
    pub(super) fn guard(&self) -> StreamedReadGuard {
        StreamedReadGuard(self.cleanup.clone())
    }

    /// 【工具流】【就绪处理】仅调度连续且通过全部门禁的文件读取，参数完整后才启动。
    /// 参数：响应顺序、调用、注册表、可见性和工作目录；返回无。
    pub(super) fn ready(
        &mut self,
        index: usize,
        call: ToolCall,
        tools: &ToolRegistry,
        visibility: &ToolVisibility,
        cwd: &PathBuf,
    ) {
        if tools.has_control_pre_hooks() || self.sealed || index != self.next {
            return;
        }
        let name = call.function.name.as_str();
        let eligible = matches!(name, "read_file" | "list_files" | "search_files" | "grep")
            && tools
                .permission_for_call(name, &call.function.arguments)
                .ok()
                == Some(ToolPermission::ReadOnly)
            && tools
                .requires_permission(name, &call.function.arguments)
                .ok()
                == Some(false)
            && evaluate_tool_gate(tools, visibility, &call) == ToolGate::Proceed;
        if !eligible || self.next >= 8 {
            self.sealed = true;
            return;
        }
        self.next += 1;
        let registry = tools.clone();
        let request = call.clone();
        let cwd = cwd.clone();
        let cancel = self.cancel.clone();
        let accounting = self.accounting.clone();
        let handle = tokio::spawn(crate::runtime_cwd::scope(cwd, async move {
            let (progress, _receiver) = tokio::sync::mpsc::channel(1);
            let prepared = registry.prepare_hooked_call(&request).await?;
            let execution = registry.execute_prepared(&prepared, progress);
            tokio::pin!(execution);
            let mut tick = tokio::time::interval(std::time::Duration::from_millis(100));
            loop {
                tokio::select! {
                    result = &mut execution => break result,
                    _ = tick.tick() => {
                        anyhow::ensure!(!cancel.load(std::sync::atomic::Ordering::Acquire), "streamed read cancelled");
                        if let Some(accounting) = &accounting {
                            anyhow::ensure!(accounting.current_goal()?.is_none_or(|goal| goal.status.is_active()), "streamed read stopped with goal");
                        }
                    }
                }
            }
        }));
        self.cleanup
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push(handle.abort_handle());
        self.pending
            .get_mut()
            .unwrap_or_else(|error| error.into_inner())
            .insert(
                call.id.clone(),
                PendingRead {
                    index,
                    call,
                    handle,
                },
            );
    }

    /// 【工具流】【响应校验】丢弃最终响应未确认或参数不一致的读取；参数为最终调用，返回无。
    pub(super) fn retain_confirmed(&mut self, calls: &[ToolCall]) {
        self.pending
            .get_mut()
            .unwrap_or_else(|error| error.into_inner())
            .retain(|_, read| {
                calls
                    .get(read.index)
                    .is_some_and(|call| same_call(call, &read.call))
            });
    }

    /// 【工具流】【结果复用】在正常门禁和调用记录完成后取回结果，等待期间取消仍终止任务。
    /// 参数：正式调用；返回对应结果，无匹配时走普通执行。
    pub(super) async fn take(&self, call: &ToolCall) -> Option<RealToolExecution> {
        let read = self
            .pending
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(&call.id);
        let mut read = read.filter(|read| same_call(call, &read.call))?;
        let result = (&mut read.handle).await;
        Some(super::tool_execution::tool_result(match result {
            Ok(result) => result,
            Err(error) => Err(error.into()),
        }))
    }
}

/// 【工具流】【身份校验】同时比较调用标识、工具和完整参数；参数为两次调用，返回是否一致。
fn same_call(first: &ToolCall, second: &ToolCall) -> bool {
    first.id == second.id
        && first.kind == second.kind
        && first.function.name == second.function.name
        && first.function.arguments == second.function.arguments
}

#[cfg(test)]
#[path = "streamed_tools_tests.rs"]
mod tests;
