//! 主会话边界独立于每轮 Agent 实例，连接断开后等待空闲期限再收尾。
use super::{HookContext, HookEvent};
use crate::{config::HooksConfig, tools::ToolRegistry};
use std::{
    collections::{HashMap, HashSet},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

struct Session {
    config: HooksConfig,
    context: HookContext,
    tools: ToolRegistry,
    active: usize,
    last_activity: Instant,
}
#[derive(Default)]
struct Lifecycle {
    sessions: HashMap<String, Session>,
    clients: HashMap<String, usize>,
    web_sessions: HashSet<String>,
    server_sessions: HashSet<String>,
    activities: HashMap<String, usize>,
}
/// 【Hooks】【会话状态】无参数，返回主会话生命周期表，客户端计数可早于首次运行建立
fn state() -> &'static Mutex<Lifecycle> {
    static STATE: OnceLock<Mutex<Lifecycle>> = OnceLock::new();
    STATE.get_or_init(Default::default)
}
/// 【Hooks】【开始判定】参数为会话，返回本进程是否已成功执行 SessionStart
pub(crate) fn started(session: &str) -> bool {
    state()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .sessions
        .contains_key(session)
}
/// 【Hooks】【开始登记】参数为有效配置、上下文和注册表，返回无值，重配置保留活动计数
pub(crate) fn register(config: HooksConfig, context: HookContext, tools: ToolRegistry) {
    let mut state = state().lock().unwrap_or_else(|error| error.into_inner());
    let active = state
        .activities
        .get(&context.session_id)
        .copied()
        .unwrap_or(0);
    state.sessions.insert(
        context.session_id.clone(),
        Session {
            config,
            context,
            tools,
            active,
            last_activity: Instant::now(),
        },
    );
}
/// 【Hooks】【活动更新】参数为会话与开始开关，返回无值，完成时刷新空闲期限
pub(crate) fn activity(session: &str, begin: bool) {
    {
        let mut state = state().lock().unwrap_or_else(|error| error.into_inner());
        let count = state.activities.entry(session.into()).or_default();
        *count = if begin {
            count.saturating_add(1)
        } else {
            count.saturating_sub(1)
        };
        let active = *count;
        if active == 0 {
            state.activities.remove(session);
        }
        if let Some(entry) = state.sessions.get_mut(session) {
            entry.active = active;
            entry.last_activity = Instant::now();
        }
    }
    if !begin {
        schedule_idle(session.to_string());
    }
}
/// 【Hooks】【结束执行】参数为会话，返回无值；先移除登记避免删除、退出与空闲收尾重复执行
pub(crate) async fn finish(session: &str) {
    let removed = {
        let mut state = state().lock().unwrap_or_else(|error| error.into_inner());
        state.server_sessions.remove(session);
        state.web_sessions.remove(session);
        state.sessions.remove(session)
    };
    if let Some(session) = removed {
        super::dispatch_with_tools(
            &session.config,
            HookEvent::SessionEnd,
            &session.context.with("reason", "other"),
            Some(&session.tools),
        )
        .await;
    }
    super::close(session);
}

/// 【Hooks】【服务收尾】无参数，等待各会话结束钩子并发完成，避免逐会话累加退出时限
pub(crate) async fn finish_all() {
    let sessions = {
        let state = state().lock().unwrap_or_else(|error| error.into_inner());
        state
            .sessions
            .keys()
            .filter(|session| state.server_sessions.contains(*session))
            .cloned()
            .collect::<Vec<_>>()
    };
    let mut pending = futures_util::stream::FuturesUnordered::new();
    for session in &sessions {
        pending.push(finish(session));
    }
    use futures_util::StreamExt;
    while pending.next().await.is_some() {}
}

/// 【Hooks】【服务归属】参数为 Web 启动的会话，返回无值，服务退出不关闭其它入口的会话
pub(crate) fn server_session(session: &str) {
    state()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .server_sessions
        .insert(session.into());
}

/// 【Hooks】【空闲判定】参数为会话，返回无值，仅关闭没有连接和活动且超过五分钟的会话
async fn expire_idle(session: &str) {
    let removed = {
        let mut state = state().lock().unwrap_or_else(|error| error.into_inner());
        let idle = state.clients.get(session).copied().unwrap_or(0) == 0
            && state.sessions.get(session).is_some_and(|entry| {
                entry.active == 0 && entry.last_activity.elapsed() >= Duration::from_secs(300)
            });
        if idle {
            state.server_sessions.remove(session);
            state.web_sessions.remove(session);
            state.sessions.remove(session)
        } else {
            None
        }
    };
    if let Some(entry) = removed {
        super::dispatch_with_tools(
            &entry.config,
            HookEvent::SessionEnd,
            &entry.context.with("reason", "other"),
            Some(&entry.tools),
        )
        .await;
        super::close(session);
    }
}
/// 【Hooks】【空闲收尾】参数为会话，返回无值；五分钟无活动且没有客户端才关闭，不终止活动轮次
fn schedule_idle(session: String) {
    if !state()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .web_sessions
        .contains(&session)
    {
        return;
    }
    if let Ok(runtime) = tokio::runtime::Handle::try_current() {
        runtime.spawn(async move {
            tokio::time::sleep(Duration::from_secs(300)).await;
            expire_idle(&session).await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn client_and_input_preparation_prevent_idle_end() {
        let id = "lifecycle-idle-test";
        activity(id, true);
        register(
            HooksConfig::default(),
            HookContext {
                session_id: id.into(),
                ..Default::default()
            },
            ToolRegistry::new(),
        );
        let client = client(id);
        state()
            .lock()
            .unwrap()
            .sessions
            .get_mut(id)
            .unwrap()
            .last_activity = Instant::now() - Duration::from_secs(301);
        expire_idle(id).await;
        assert!(started(id));
        drop(client);
        expire_idle(id).await;
        assert!(started(id));
        activity(id, false);
        state()
            .lock()
            .unwrap()
            .sessions
            .get_mut(id)
            .unwrap()
            .last_activity = Instant::now() - Duration::from_secs(301);
        expire_idle(id).await;
        assert!(!started(id));
    }
}
pub(crate) struct ClientGuard(String);
/// 【Hooks】【客户端连接】参数为会话，返回与 SSE 流绑定的连接守卫
pub(crate) fn client(session: &str) -> ClientGuard {
    let mut state = state().lock().unwrap_or_else(|error| error.into_inner());
    state.web_sessions.insert(session.into());
    *state.clients.entry(session.into()).or_default() += 1;
    ClientGuard(session.into())
}
impl Drop for ClientGuard {
    /// 【Hooks】【客户端断开】无参数和返回值，最后连接断开后安排空闲检查
    fn drop(&mut self) {
        let mut state = state().lock().unwrap_or_else(|error| error.into_inner());
        if let Some(count) = state.clients.get_mut(&self.0) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                state.clients.remove(&self.0);
            }
        }
        drop(state);
        schedule_idle(self.0.clone());
    }
}
