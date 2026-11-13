//! 每个会话最多八项在执行、六十四项在途，异步输出只作为后续上下文。
use super::{HookContext, HookEvent};
use crate::{config::HookItem, tools::ToolRegistry};
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex, OnceLock},
};
use tokio::{sync::Semaphore, task::JoinHandle};
struct Tasks {
    semaphore: Arc<Semaphore>,
    jobs: Vec<JoinHandle<()>>,
    messages: VecDeque<String>,
}
/// 【Hooks】【后台状态】无参数，返回会话隔离的任务表
fn tasks() -> &'static Mutex<HashMap<String, Tasks>> {
    static TASKS: OnceLock<Mutex<HashMap<String, Tasks>>> = OnceLock::new();
    TASKS.get_or_init(|| Mutex::new(HashMap::new()))
}
/// 【Hooks】【后台调度】参数为动作、事件、上下文与 MCP 注册表，返回无值，队列满时记录错误
pub(super) fn spawn(
    hook: HookItem,
    event: HookEvent,
    context: HookContext,
    tools: Option<ToolRegistry>,
) {
    let mut tasks = tasks().lock().unwrap_or_else(|error| error.into_inner());
    let entry = tasks
        .entry(context.session_id.clone())
        .or_insert_with(|| Tasks {
            semaphore: Arc::new(Semaphore::new(8)),
            jobs: Vec::new(),
            messages: VecDeque::new(),
        });
    entry.jobs.retain(|job| !job.is_finished());
    if entry.jobs.len() >= 64 {
        eprintln!("【Hooks】【后台队列】会话队列已满，跳过 {}", hook.name);
        return;
    }
    let semaphore = entry.semaphore.clone();
    entry.jobs.push(tokio::spawn(async move {
        let Ok(_permit) = semaphore.acquire_owned().await else {
            return;
        };
        match super::run(&hook, event, &context, tools.as_ref()).await {
            Ok(result) => {
                let mut map = self::tasks()
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                if let Some(entry) = map.get_mut(&context.session_id) {
                    for text in result.context.into_iter().chain(result.messages) {
                        if entry.messages.len() >= 32 {
                            entry.messages.pop_front();
                        }
                        entry
                            .messages
                            .push_back(text.chars().take(16_384).collect());
                    }
                }
            }
            Err(error) => eprintln!("【Hooks】【后台失败】{}: {error}", hook.name),
        }
    }));
}
/// 【Hooks】【后台反馈】参数为会话，返回已完成的反馈并清空送达队列
pub(super) fn drain(session: &str) -> Vec<String> {
    tasks()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .get_mut(session)
        .map(|tasks| tasks.messages.drain(..).collect())
        .unwrap_or_default()
}
/// 【Hooks】【后台取消】参数为会话，移除状态并取消全部在途任务，无返回值
pub(super) fn close(session: &str) {
    if let Some(tasks) = tasks()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .remove(session)
    {
        for job in tasks.jobs {
            job.abort();
        }
    }
}
