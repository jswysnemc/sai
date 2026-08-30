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
            "Inspect Sai sessions and who holds them, without touching them. scope=self (default) reports only the current session; workspace reports every session in this workspace; all reports sessions across every workspace. Each entry carries the session id and title, whether it is its workspace's current session, the holder (owner kind, pid, whether the holder is alive, watcher count, event transport) and whether a turn is running right now. Use it before coordinating across sessions: pick a session that is held but idle, and remember that only the holder can drive a session. Read-only: it never starts, switches, or interrupts a session.",
            "查看 Sai 会话及其持有者,不碰任何会话。scope=self(默认)只报当前会话;workspace 报本工作区的全部会话;all 报所有工作区的会话。每条记录包含会话 id 与标题、是否为所在工作区的当前会话、持有者(owner 类型、pid、是否存活、观察者数、事件端点)以及此刻是否正在跑一轮。跨会话协作前先查它:挑一个已被持有但空闲的会话,并且记住只有持有者能驱动会话。只读工具:不会启动、切换或打断会话。",
        ),
        json!({
            "type": "object",
            "properties": {
                "scope": {
                    "type": "string",
                    "enum": ["self", "workspace", "all"],
                    "description": t(
                        "Which sessions to report: self (current session), workspace (all sessions in this workspace), or all (sessions in every workspace). Defaults to self.",
                        "要查看的范围:self(当前会话)、workspace(本工作区的全部会话)、all(所有工作区的会话)。默认 self。"
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
    let scope = scope_arg(&args, &["self", "workspace", "all"], "self")?;
    let sessions = sessions_in_scope(&context, &scope)?;
    let sessions = sessions
        .iter()
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
        "sessions": sessions,
    }))?)
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
        "running": running_turn.is_some(),
        "holder": holder,
        "active_run": running_turn,
    })
}
