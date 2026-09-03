use super::{optional_string_arg, MeshContext};
use crate::i18n::text as t;
use crate::tools::{ToolRegistry, ToolSpec};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

/// 英文版工具说明。
pub(super) const DESCRIPTION_EN: &str = "Activate a session: point its workspace's current-session pointer at it so the user can /resume it. Resolve the target by session_id (searches every workspace) or by name (its title). Name lookup only matches sessions with an explicitly set title and only when exactly one session has that title — placeholder titles (New session, Default) never match, and an ambiguous name returns the candidate ids instead of guessing. Returns the session_id and the wire address session:<session_id> for subsequent mesh_send calls. Activating a session in another workspace requires mesh.cross_session=true. Activation only moves the pointer: it does not start or interrupt anything, and it does not switch this running agent to the target session.";

/// 中文版工具说明。
pub(super) const DESCRIPTION_ZH: &str = "激活一个会话：把它所在工作区的当前会话指针指向它，用户随后可以 /resume 进入。用 session_id（跨所有工作区查找）或 name（标题）定位目标。name 只匹配被显式命名的会话，且仅当恰好一条会话使用该标题——占位标题（New session、Default）永不匹配，重名时返回候选 id 而不是瞎猜。返回 session_id 与线格式地址 session:<session_id>，供后续 mesh_send 使用。激活其它工作区的会话需要 mesh.cross_session=true。激活只移动指针：它本身不会启动或打断任何会话，也不会把当前运行中的 Agent 切换到目标会话。";

/// 注册 session_activate 工具。
///
/// 参数:
/// - `registry`: 工具注册表
/// - `context`: 网格上下文
///
/// 返回:
/// - 无
pub(super) fn register(registry: &mut ToolRegistry, context: MeshContext) {
    registry.register(
        ToolSpec::new(
            "session_activate",
            t(DESCRIPTION_EN, DESCRIPTION_ZH),
            json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": t(
                            "Session id to activate. Takes precedence over name; pass either one, not both.",
                            "要激活的会话 id。与 name 二选一，不要同时传。"
                        )
                    },
                    "name": {
                        "type": "string",
                        "description": t(
                            "Explicit session title to activate. Only unique titles match; ambiguous names list candidate ids.",
                            "要激活的会话显式标题。只有唯一的标题才能匹配；重名时会列出候选 id。"
                        )
                    }
                },
                "additionalProperties": false
            }),
            move |args| {
                let context = context.clone();
                async move { activate(context, args).await }
            },
        )
        .writes(),
    );
}

/// 激活目标会话：把其所在工作区的当前指针指向它。
///
/// 参数:
/// - `context`: 网格上下文
/// - `args`: 工具参数
///
/// 返回:
/// - JSON 形式的激活结果
pub(super) async fn activate(context: MeshContext, args: Value) -> Result<String> {
    let target = resolve_target(&context, &args)?;
    // 归属校验：激活别的工作区的会话和跨会话投递同等越界，需要显式开关
    let current_workspace = crate::state::current_workspace_id()?;
    if target.workspace_id != current_workspace && !context.cross_session {
        bail!(
            "session {} lives in another workspace; set mesh.cross_session=true to activate it",
            target.session_id
        );
    }
    let session = crate::state::switch_session_located(&context.paths, &target.session_id)?;
    Ok(serde_json::to_string_pretty(&json!({
        "ok": true,
        "session_id": session.id,
        "title": session.title,
        "workspace_id": target.workspace_id,
        "address": format!("session:{}", session.id),
        "note": t(
            "Activated: the session is now its workspace's current session and the user can /resume it. Deliver work with mesh_send to the address above.",
            "已激活：该会话现在是其工作区的当前会话，用户可以 /resume 进入。用 mesh_send 向上面的地址投递工作内容。"
        ),
    }))?)
}

/// 解析出的激活目标。
struct ActivateTarget {
    session_id: String,
    workspace_id: String,
}

/// 从工具参数解析激活目标：session_id 或 name 二选一。
///
/// 参数:
/// - `context`: 网格上下文
/// - `args`: 工具参数
///
/// 返回:
/// - 目标会话 id 及其所属工作区
fn resolve_target(context: &MeshContext, args: &Value) -> Result<ActivateTarget> {
    let id = optional_string_arg(args, "session_id");
    let name = optional_string_arg(args, "name");
    match (id, name) {
        (Some(session_id), None) => locate_by_id(context, &session_id),
        (None, Some(name)) => locate_by_name(context, &name),
        (Some(_), Some(_)) => bail!("pass either session_id or name, not both"),
        (None, None) => bail!("missing required argument: session_id or name"),
    }
}

/// 按 id 定位会话及其所属工作区。
///
/// 参数:
/// - `context`: 网格上下文
/// - `session_id`: 会话 id
///
/// 返回:
/// - 激活目标
fn locate_by_id(context: &MeshContext, session_id: &str) -> Result<ActivateTarget> {
    let (base, _) = crate::state::locate_session_dirs(&context.paths, session_id)?;
    let workspace_id = base
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .context("session scope directory has no workspace id")?;
    Ok(ActivateTarget {
        session_id: session_id.to_string(),
        workspace_id,
    })
}

/// 按显式名称定位会话：占位标题不参与匹配，重名时列出候选 id。
///
/// 参数:
/// - `context`: 网格上下文
/// - `name`: 会话显式标题
///
/// 返回:
/// - 唯一匹配的激活目标
fn locate_by_name(context: &MeshContext, name: &str) -> Result<ActivateTarget> {
    let matches = crate::state::list_all_sessions(&context.paths)?
        .into_iter()
        .filter(|session| title_is_explicit_name(&session.info.title) && session.info.title == name)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => bail!(
            "no session named {name:?} (placeholder titles never match); pass session_id or use session_probe to list sessions"
        ),
        [only] => Ok(ActivateTarget {
            session_id: only.info.id.clone(),
            workspace_id: only.workspace_id.clone(),
        }),
        many => {
            let candidates = many
                .iter()
                .map(|session| format!("{} ({})", session.info.id, session.info.title))
                .collect::<Vec<_>>()
                .join(", ");
            bail!(
                "session name {name:?} is ambiguous across {} sessions; pass one of these session ids instead: {candidates}",
                many.len()
            )
        }
    }
}

/// 判断标题是否为用户显式设置的名称。
///
/// `New session` 是未命名占位，`Default` 是默认会话占位；两者都会被
/// 自动标题逻辑改写，不能作为稳定名称匹配。
///
/// 参数:
/// - `title`: 会话标题
///
/// 返回:
/// - 显式名称为 true
fn title_is_explicit_name(title: &str) -> bool {
    title != "New session" && title != "Default"
}
