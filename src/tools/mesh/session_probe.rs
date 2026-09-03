use super::{scope_arg, sessions_in_scope, MeshContext};
use crate::i18n::text as t;
use crate::runner::{active_run, holder_is_alive, session_holder};
use crate::state::LocatedSession;
use crate::tools::{ToolRegistry, ToolSpec};
use anyhow::Result;
use serde_json::{json, Value};
use std::path::Path;

/// 英文版工具说明。
pub(super) const DESCRIPTION_EN: &str = "Inspect live Sai sessions and who holds them, without touching them. scope=all (default) reports live sessions across every workspace; workspace reports this workspace; self reports only the current session. A session is live when a terminal, web page, or gateway holds it — including a brand-new session that has not received any prompt yet. Inactive sessions (no live holder) are omitted, except the current session itself. The sessions array lists ONLY OTHER sessions — your own session is never in it, so any entry in sessions is a valid mesh_send target; your identity is in the top-level self.session_id. Each entry carries the session id and title, whether it is idle (held but not running a turn), the holder, and whether a turn is running. Never use is_workspace_current to decide \"which session am I\": it only names the session this workspace's shared pointer file points at, which is often owned by another terminal or tab. Use it before coordinating across sessions. Read-only: it never starts, switches, or interrupts a session.";

/// 中文版工具说明。
pub(super) const DESCRIPTION_ZH: &str = "查看仍在活动的 Sai 会话及其持有者,不碰任何会话。scope=all(默认)报所有工作区的活动会话;workspace 报本工作区;self 只报当前会话。终端、网页或网关打开着的会话就算活动,包括还没有发过任何提示词的新会话。没有存活持有者的非活动会话不展示,当前会话本身除外。sessions 数组只列出其它会话——你自己的会话绝不会出现在里面,因此 sessions 里任意一条都是合法的 mesh_send 目标;你的身份看顶层 self.session_id。每条记录包含会话 id 与标题、是否空闲(已打开但没在跑一轮)、持有者以及此刻是否正在跑一轮。绝不要用 is_workspace_current 判断「哪个是我」:它只说明本工作区共享指针文件指向哪条会话,那个会话常常属于另一个终端或标签页。跨会话协作前先查它。只读工具:不会启动、切换或打断会话。";

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
        t(DESCRIPTION_EN, DESCRIPTION_ZH),
        json!({
            "type": "object",
            "properties": {
                "scope": {
                    "type": "string",
                    "enum": ["self", "workspace", "all"],
                    "description": t(
                        "Which live sessions to report: all (every workspace, default), workspace (this workspace), or self (your own session — the entry with is_self: true). Inactive sessions are omitted.",
                        "要查看的活动会话范围:all(所有工作区,默认)、workspace(本工作区)、self(你自己的会话,即 is_self 为 true 的那条)。非活动会话不展示。"
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
    let mut self_entry = None;
    let mut sessions = Vec::new();
    for session in located.iter().filter(|session| session_is_active(session, &context)) {
        // 自己那条从 sessions 里分出去：模型遍历列表挑目标时不再可能选中自己。
        // scope=self 除外——那种情况列表本来就只有自己
        if scope != "self" && Path::new(&context.owner_key) == session.state_dir.as_path() {
            self_entry = Some(describe(session, &context));
        } else {
            sessions.push(describe(session, &context));
        }
    }
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "scope": scope,
        "self": {
            "session_id": context.session_id,
            "state_dir": context.owner_key,
        },
        "count": sessions.len(),
        "self_included": self_entry.is_some(),
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
        // 工作区共享指针文件的答案：它只说明「这个工作区当前指向哪条会话」，
        // 那个会话可能属于另一个终端，不能拿来判定「哪个是我」
        "is_workspace_current": session.is_current,
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
