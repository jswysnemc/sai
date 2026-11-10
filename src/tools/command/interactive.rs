pub(crate) mod management;
mod output;
mod session;
#[cfg(test)]
mod tests;

use super::background_tasks::BackgroundRuntimeOwner;
use crate::config::AppConfig;
use crate::sandbox::{FileAccess, SandboxPolicy};
use crate::tools::{ToolRegistry, ToolSpec};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use session::InteractiveSession;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

const MAX_SESSIONS_PER_OWNER: usize = 16;
const MAX_SESSIONS: usize = 64;
type Sessions = HashMap<String, (Arc<InteractiveSession>, Instant)>;
static SESSIONS: OnceLock<Mutex<Sessions>> = OnceLock::new();

/// 【交互命令】【会话存储】获取进程内会话注册表；无参数，返回互斥表。
fn sessions() -> &'static Mutex<Sessions> {
    SESSIONS.get_or_init(Mutex::default)
}

/// 【交互命令】【归属标识】区分交互会话和命令模式；参数为运行归属，返回稳定键。
fn owner_key(owner: &BackgroundRuntimeOwner) -> String {
    format!("{}:{}", owner.owner_kind.as_str(), owner.owner_id)
}

/// 【交互命令】【输入工具】注册仅能访问所属会话的输入工具。
/// 参数：注册表、应用配置与可选运行归属；返回无。
pub(super) fn register(
    registry: &mut ToolRegistry,
    config: &AppConfig,
    owner: Option<BackgroundRuntimeOwner>,
    allowed: bool,
) {
    if !config.tools.enabled {
        return;
    }
    registry.register(ToolSpec::new("write_stdin",
        "Send chars to an interactive run_command session or poll new output with empty chars. Sessions belong to the current conversation. Use terminate=true to stop the entire process tree. A completed result removes the session.",
        json!({"type":"object","properties":{
            "session_id":{"type":"string","description":"session_id returned by run_command with tty=true"},
            "chars":{"type":"string","description":"Text to write, including newline when submitting a line; empty polls output"},
            "yield_time_ms":{"type":"integer","minimum":0,"maximum":60000},
            "terminate":{"type":"boolean"}
        },"required":["session_id"],"additionalProperties":false}),
        move |args| {
            let owner = owner.clone();
            async move {
                anyhow::ensure!(allowed, "command execution is disabled");
                let owner = owner.context("interactive command sessions require a bound conversation")?;
                interact(&owner_key(&owner), &args).await
            }
        }).writes());
}

/// 【交互命令】【启动入口】启动 PTY 并等待首批输出，取消启动调用会回收进程树。
/// 参数：工具参数、Shell 与归属；返回包含 session_id 或退出码的 JSON。
pub(super) async fn start(
    args: &Value,
    shell: &str,
    owner: Option<&BackgroundRuntimeOwner>,
) -> Result<String> {
    let owner = owner.context("interactive command sessions require a bound conversation")?;
    let command = args["command"].as_str().context("command is required")?;
    let cwd = args["cwd"]
        .as_str()
        .map(std::path::PathBuf::from)
        .unwrap_or(crate::runtime_cwd::current_dir()?);
    anyhow::ensure!(cwd.is_dir(), "interactive working directory does not exist");
    let policy = args["_sai_sandbox"]
        .as_bool()
        .unwrap_or(false)
        .then(|| SandboxPolicy::for_current_workspace(FileAccess::WorkspaceWrite, Some(&cwd)))
        .transpose()?;
    let owner = owner_key(owner);
    let id = format!("pty_{}", uuid::Uuid::new_v4().simple());
    let session = {
        let mut sessions = sessions()
            .lock()
            .map_err(|_| anyhow::anyhow!("interactive session lock poisoned"))?;
        // 1. 【交互命令】【容量限制】保留最近完成输出，清除过期记录；不会为腾出容量杀死运行任务
        sessions.retain(|_, (session, created)| {
            !(created.elapsed() > Duration::from_secs(600)
                && session
                    .status()
                    .is_ok_and(|(code, eof)| code.is_some() && eof))
        });
        anyhow::ensure!(
            sessions.len() < MAX_SESSIONS
                && sessions
                    .values()
                    .filter(|(session, _)| session.owner == owner)
                    .count()
                    < MAX_SESSIONS_PER_OWNER,
            "too many interactive command sessions; finish or terminate existing sessions first"
        );
        let session = Arc::new(InteractiveSession::spawn(
            owner,
            command,
            shell,
            &cwd,
            policy.as_ref(),
        )?);
        sessions.insert(id.clone(), (session.clone(), Instant::now()));
        session
    };
    // 2. 【交互命令】【进程回收】独立观察退出，避免未继续轮询的子进程长期成为僵尸
    let weak = Arc::downgrade(&session);
    tokio::spawn(async move {
        loop {
            let done = match weak.upgrade() {
                Some(session) => session.status().is_ok_and(|(code, _)| code.is_some()),
                None => true,
            };
            if done {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });
    let mut guard = OperationGuard {
        id: id.clone(),
        session: session.clone(),
        armed: true,
    };
    let wait = args["yield_time_ms"].as_u64().unwrap_or(1000).min(60_000);
    let result = collect(&id, &session, wait).await?;
    guard.armed = false;
    Ok(result)
}

/// 【交互命令】【继续入口】校验归属后串行写入与消费输出，不允许跨会话注入输入。
/// 参数：所属会话键与工具参数；返回新输出及运行状态。
async fn interact(owner: &str, args: &Value) -> Result<String> {
    let id = args["session_id"]
        .as_str()
        .context("session_id is required")?;
    let session = sessions()
        .lock()
        .map_err(|_| anyhow::anyhow!("interactive session lock poisoned"))?
        .get(id)
        .map(|(session, _)| session.clone())
        .filter(|session| session.owner == owner)
        .context("interactive session not found in current conversation")?;
    let _operation = session.operation.lock().await;
    let chars = args["chars"].as_str().unwrap_or_default();
    anyhow::ensure!(chars.len() <= 64 * 1024, "interactive input exceeds 64 KiB");
    let mut guard = OperationGuard {
        id: id.to_string(),
        session: session.clone(),
        armed: true,
    };
    if args["terminate"].as_bool().unwrap_or(false) {
        session.terminate();
    } else if !chars.is_empty() && session.status()?.0.is_none() {
        let writer = session.clone();
        let input = chars.to_string();
        tokio::task::spawn_blocking(move || writer.write(&input)).await??;
    }
    let wait = args["yield_time_ms"].as_u64().unwrap_or(1000).min(60_000);
    let result = collect(id, &session, wait).await?;
    guard.armed = false;
    Ok(result)
}

/// 【交互命令】【输出交付】有界等待进程退出，完成后只在输出排空时移除会话。
/// 参数：标识、会话与最大等待毫秒数；返回新的输出片段及状态 JSON。
async fn collect(id: &str, session: &InteractiveSession, wait_ms: u64) -> Result<String> {
    let deadline = tokio::time::Instant::now() + Duration::from_millis(wait_ms);
    let (exit_code, eof) = loop {
        let status = session.status()?;
        if status.0.is_some() && status.1 || tokio::time::Instant::now() >= deadline {
            break status;
        }
        tokio::time::sleep(
            Duration::from_millis(20)
                .min(deadline.saturating_duration_since(tokio::time::Instant::now())),
        )
        .await;
    };
    let (output, omitted_bytes) = session.take_output();
    let finished = exit_code.is_some() && eof;
    if finished {
        sessions()
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(id);
    }
    Ok(serde_json::to_string_pretty(&json!({
        "mode":"interactive", "session_id": if finished { None } else { Some(id) },
        "pid":session.pid,"running":!finished,"exit_code":exit_code,"stdout":output,"stderr":"",
        "omitted_bytes":omitted_bytes,"success":exit_code.is_none_or(|code| code == 0),
        "note":if finished { "Command completed; session removed." } else { "Use write_stdin with this session_id to send input, poll output, or terminate." }
    }))?)
}

/// 【交互命令】【取消守卫】未成功返回的交互调用释放整个进程树及注册记录。
struct OperationGuard {
    id: String,
    session: Arc<InteractiveSession>,
    armed: bool,
}
impl Drop for OperationGuard {
    /// 【交互命令】【取消执行】取消工具等待或写入时清理命令资源；无参数、无返回值。
    fn drop(&mut self) {
        if self.armed {
            self.session.terminate();
            sessions()
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .remove(&self.id);
        }
    }
}
