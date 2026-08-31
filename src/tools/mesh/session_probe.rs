use super::{scope_arg, sessions_in_scope, MeshContext};
use crate::i18n::text as t;
use crate::runner::{active_run, holder_is_alive, session_holder};
use crate::state::LocatedSession;
use crate::tools::{ToolRegistry, ToolSpec};
use anyhow::Result;
use serde_json::{json, Value};
use std::path::Path;

/// 注册 session_probe 工具。
///
/// 参数:
/// - `registry`: 工具注册表
/// - `context`: 网格探测上下文
///
/// 返回:
/// - 无
pub(super) fn register(registry: &mut ToolRegistry, context: MeshContext) {
    registry.register(ToolSpec::new(
        "session_probe",
        t(
            "Inspect live Sai sessions and who holds them, without touching them. scope=all (default) reports live sessions across every workspace; workspace reports this workspace; self reports only the current session. A session is live when a terminal, web page, or gateway holds it — including a brand-new session that has not received any prompt yet. Inactive sessions (no live holder) are omitted, except the current session itself. Each entry carries the session id and title, whether it is idle (held but not running a turn), whether it is its workspace's current session, the holder, and whether a turn is running. Use it before coordinating across sessions. Read-only: it never starts, switches, or interrupts a session.",
            "查看仍在活动的 Sai 会话及其持有者,不碰任何会话。scope=all(默认)报所有工作区的活动会话;workspace 报本工作区;self 只报当前会话。终端、网页或网关打开着的会话就算活动,包括还没有发过任何提示词的新会话。没有存活持有者的非活动会话不展示,当前会话本身除外。每条记录包含会话 id 与标题、是否空闲(已打开但没在跑一轮)、是否为所在工作区的当前会话、持有者以及此刻是否正在跑一轮。跨会话协作前先查它。只读工具:不会启动、切换或打断会话。",
        ),
        json!({
            "type": "object",
            "properties": {
                "scope": {
                    "type": "string",
                    "enum": ["self", "workspace", "all"],
                    "description": t(
                        "Which live sessions to report: all (every workspace, default), workspace (this workspace), or self (current session). Inactive sessions are omitted.",
                        "要查看的活动会话范围:all(所有工作区,默认)、workspace(本工作区)、self(当前会话)。非活动会话不展示。"
                    )
                }
            },
            "additionalProperties": false
        }),
        move |args| {
            let context = context.clone();
            async move { probe(context, args).await }
        },
    ));
}

/// 探测作用域内的会话持有情况。
///
/// 参数:
/// - `context`: 网格探测上下文
/// - `args`: 工具参数
///
/// 返回:
/// - JSON 形式的会话列表
pub(super) async fn probe(context: MeshContext, args: Value) -> Result<String> {
    let scope = scope_arg(&args, &["self", "workspace", "all"], "all")?;
    let located = sessions_in_scope(&context, &scope)?;
    let total = located.len();
    let sessions = located
        .iter()
        .filter(|session| session_is_active(session, &context))
        .map(|session| describe(session, &context))
        .collect::<Vec<_>>();
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "scope": scope,
        "self": {
            "session_id": context.session_id,
            "state_dir": context.owner_key,
        },
        "count": sessions.len(),
        "omitted_inactive": total.saturating_sub(sessions.len()),
        "sessions": sessions,
    }))?)
}

/// 判断探测结果是否应展示该会话。
///
/// 当前会话始终展示；其它会话只在持有者进程仍存活时展示。
///
/// 参数:
/// - `session`: 已定位的会话
/// - `context`: 网格探测上下文
///
/// 返回:
/// - 活动会话为 true
fn session_is_active(session: &LocatedSession, context: &MeshContext) -> bool {
    Path::new(&context.owner_key) == session.state_dir.as_path()
        || session_holder(Path::new(&session.state_dir))
            .is_some_and(|record| holder_is_alive(&record))
}

/// 描述单个会话的持有者与运行状态。
///
/// 参数:
/// - `session`: 已定位的会话
/// - `context`: 网格探测上下文
///
/// 返回:
/// - 会话探测结果
fn describe(session: &LocatedSession, context: &MeshContext) -> Value {
    let state_dir = Path::new(&session.state_dir);
    let holder = session_holder(state_dir).map(|record| {
        json!({
            "owner": record.owner,
            "pid": record.pid,
            "alive": holder_is_alive(&record),
            "watchers": record.watchers,
            "transport": record.transport,
            "held_since": record.started_at,
            "heartbeat_at": record.heartbeat_at,
        })
    });
    let running_turn = active_run(state_dir).map(|record| {
        json!({
            "owner": record.owner,
            "pid": record.pid,
            "started_at": record.started_at,
        })
    });
    json!({
        "id": session.info.id,
        "title": session.info.title,
        "workspace_id": session.workspace_id,
        "state_dir": session.state_dir.display().to_string(),
        "updated_at": session.info.updated_at,
        "is_current": session.is_current,
        // 每个工作区都有 default 会话，按 id 判断会把别的工作区的 default
        // 也当成自己；状态目录才是会话的唯一身份
        "is_self": Path::new(&context.owner_key) == session.state_dir,
        "held": holder.is_some(),
        "idle": running_turn.is_none(),
        "running": running_turn.is_some(),
        "holder": holder,
        "active_run": running_turn,
    })
}
