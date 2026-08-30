use super::{optional_string_arg, sessions_in_scope, unix_seconds, MeshContext};
use crate::i18n::text as t;
use crate::runner::{holder_is_alive, session_holder};
use crate::state::LocatedSession;
use crate::tools::subagent_state::{list_subagents_for_owner, SubagentSnapshot};
use crate::tools::{ToolRegistry, ToolSpec};
use anyhow::Result;
use serde_json::{json, Value};
use std::path::Path;

/// 注册 agent_probe 工具。
///
/// 参数:
/// - `registry`: 工具注册表
/// - `context`: 网格探测上下文
///
/// 返回:
/// - 无
pub(super) fn register(registry: &mut ToolRegistry, context: MeshContext) {
    registry.register(ToolSpec::new(
        "agent_probe",
        t(
            "Inspect subagents without disturbing them. scope=self (default) lists the current session's subagents, owner lists subagents of every session held by this process, all lists subagents of every session on disk including those owned by other processes. Pass agent_id to look up one subagent. Each entry reports type, status, step, last tool, elapsed seconds, token usage, and queued messages. Read-only: use the subagent tool to start, message, or cancel one.",
            "查看子智能体而不打扰它们。scope=self(默认)列出当前会话的子智能体,owner 列出本进程持有的所有会话的子智能体,all 列出磁盘上所有会话的子智能体(含其它进程持有的)。传 agent_id 可精确查一个。每条记录包含类型、状态、步数、最近工具、已耗时、token 用量与待处理消息数。只读工具:启动、发消息、取消请用 subagent 工具。",
        ),
        json!({
            "type": "object",
            "properties": {
                "scope": {
                    "type": "string",
                    "enum": ["self", "owner", "all"],
                    "description": t("self = current session, owner = every session held by this process, all = every session on disk. Defaults to self.", "self=当前会话,owner=本进程持有的所有会话,all=磁盘上的所有会话。默认 self。")
                },
                "agent_id": {
                    "type": "string",
                    "description": t("Return only the subagent with this id.", "只返回该 id 对应的子智能体。")
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

/// 探测作用域内会话的子智能体。
///
/// 参数:
/// - `context`: 网格探测上下文
/// - `args`: 工具参数
///
/// 返回:
/// - JSON 形式的子智能体清单
pub(super) async fn probe(context: MeshContext, args: Value) -> Result<String> {
    let scope = super::scope_arg(&args, &["self", "owner", "all"], "self")?;
    // owner 与 all 都要跨会话扫描，区别只在于是否过滤到本进程持有的会话
    let sessions = sessions_in_scope(&context, if scope == "self" { "self" } else { "all" })?;
    let sessions = match scope.as_str() {
        "owner" => sessions
            .into_iter()
            .filter(|session| held_by_this_process(session, &context))
            .collect::<Vec<_>>(),
        _ => sessions,
    };

    let mut agents = Vec::new();
    for session in &sessions {
        agents.extend(describe_session_agents(session, &context));
    }
    let only_agent = optional_string_arg(&args, "agent_id");
    let agents = match &only_agent {
        Some(agent_id) => agents
            .into_iter()
            .filter(|agent| agent["agent_id"] == *agent_id)
            .collect::<Vec<_>>(),
        None => agents,
    };
    if only_agent.is_some() && agents.is_empty() {
        return Ok(serde_json::to_string_pretty(&json!({
            "ok": false,
            "scope": scope,
            "agents": [],
            "message": t("subagent not found in this scope", "该作用域内没有这个子智能体"),
        }))?);
    }
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "scope": scope,
        "self": {
            "session_id": context.session_id,
            "state_dir": context.owner_key,
        },
        "count": agents.len(),
        "agents": agents,
    }))?)
}

/// 判断会话是否由当前进程持有。
///
/// 当前会话即使还没有登记持有者也算本进程的；其它会话按持有者登记的 pid 判断。
///
/// 参数:
/// - `session`: 已定位的会话
/// - `context`: 网格探测上下文
///
/// 返回:
/// - 是否由当前进程持有
fn held_by_this_process(session: &LocatedSession, context: &MeshContext) -> bool {
    Path::new(&context.owner_key) == session.state_dir.as_path()
        || session_holder(Path::new(&session.state_dir))
            .is_some_and(|record| record.pid == std::process::id())
}

/// 列出单个会话的子智能体。
///
/// 本进程持有的会话读内存状态（最新），其它会话只读其持久化文件，
/// 避免探测行为改写别的会话的子智能体状态。
///
/// 参数:
/// - `session`: 已定位的会话
/// - `context`: 网格探测上下文
///
/// 返回:
/// - 该会话的子智能体探测结果
fn describe_session_agents(session: &LocatedSession, context: &MeshContext) -> Vec<Value> {
    let state_dir = Path::new(&session.state_dir);
    let in_process = held_by_this_process(session, context);
    let owner_key = session.state_dir.display().to_string();
    let snapshots: Vec<SubagentSnapshot> = if in_process {
        list_subagents_for_owner(&owner_key)
    } else {
        crate::tools::subagent_persistence::load(&owner_key)
            .unwrap_or_default()
            .into_iter()
            .map(|record| record.snapshot)
            .collect()
    };
    // 当前进程自己持有的会话可能还没登记持有者（登记晚于工具注册），
    // 没有登记时按是否本进程持有判断存活，否则自己的会话会被报成失联
    let holder_alive = match session_holder(state_dir) {
        Some(record) => holder_is_alive(&record),
        None => in_process,
    };
    snapshots
        .iter()
        .map(|snapshot| describe_agent(session, snapshot, holder_alive))
        .collect()
}

/// 汇总单个子智能体的运行信息。
///
/// 参数:
/// - `session`: 子智能体所属会话
/// - `snapshot`: 子智能体快照
/// - `holder_alive`: 所属会话的持有者进程是否存活
///
/// 返回:
/// - 单个子智能体的探测结果
fn describe_agent(
    session: &LocatedSession,
    snapshot: &SubagentSnapshot,
    holder_alive: bool,
) -> Value {
    let now = unix_seconds();
    let total_tokens = snapshot
        .stats
        .as_ref()
        .and_then(|stats| stats.get("total_tokens"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    json!({
        "agent_id": snapshot.id,
        "session_id": session.info.id,
        "workspace_id": session.workspace_id,
        "type": snapshot.subagent_type,
        "description": snapshot.description,
        "status": snapshot.status,
        "step": snapshot.step,
        "max_steps": snapshot.max_steps,
        "phase": snapshot.phase,
        "last_tool": snapshot.last_tool,
        "elapsed_seconds": now.saturating_sub(snapshot.started_at),
        "updated_seconds_ago": now.saturating_sub(snapshot.updated_at),
        "total_tokens": total_tokens,
        "stats": snapshot.stats,
        "pending_messages": snapshot.pending_messages,
        "turns_completed": snapshot.turns_completed,
        "persistent": snapshot.persistent,
        "goal_id": snapshot.goal_id,
        "holder_alive": holder_alive,
        "error": snapshot.error,
    })
}
