mod address;
mod agent_probe;
mod mailbox;
mod send;
mod session_probe;
#[cfg(test)]
mod tests;

use crate::paths::SaiPaths;
use crate::state::LocatedSession;
use crate::tools::ToolRegistry;
pub(crate) use address::{session_scope_for_call, MeshAddress};
use anyhow::{anyhow, bail, Result};
pub(crate) use mailbox::{acknowledge as acknowledge_mesh_messages, next_pending, MeshEnvelope};
use serde_json::Value;
use std::path::Path;

/// 网格工具上下文。
///
/// 两个探测工具是只读的：只读取会话索引、持有者登记、轮次锁和子智能体
/// 持久化文件，不创建也不修改任何状态。收发工具会往目标会话的信箱里
/// 写消息，因此受 `cross_session` 归属开关约束。接收不再单独提供工具：
/// 投递后由会话的外部事件队列主动回执给主 Agent。
#[derive(Clone)]
pub(crate) struct MeshContext {
    paths: SaiPaths,
    /// 当前会话状态目录，同时是子智能体的 owner_key
    owner_key: String,
    session_id: String,
    /// 是否允许跨越会话边界投递（来自 `mesh.cross_session`）
    cross_session: bool,
}

/// 注册网格工具。
///
/// 参数:
/// - `registry`: 工具注册表
/// - `paths`: Sai 路径
/// - `owner_key`: 当前会话状态目录
/// - `session_id`: 当前会话标识
/// - `cross_session`: 是否允许跨会话投递
///
/// 返回:
/// - 无
pub(crate) fn register(
    registry: &mut ToolRegistry,
    paths: SaiPaths,
    owner_key: String,
    session_id: String,
    cross_session: bool,
) {
    let context = MeshContext {
        paths,
        owner_key,
        session_id,
        cross_session,
    };
    session_probe::register(registry, context.clone());
    agent_probe::register(registry, context.clone());
    send::register(registry, context);
}

/// 返回当前会话自己的地址。
///
/// 参数:
/// - `context`: 网格上下文
///
/// 返回:
/// - 当前会话的地址
pub(super) fn self_address(context: &MeshContext) -> MeshAddress {
    MeshAddress::Session(context.session_id.clone())
}

/// 校验目标地址是否允许投递。
///
/// 这是归属隔离的落点：默认只允许投给当前会话自己（含它名下的子智能体），
/// 跨会话目标必须显式开启 `mesh.cross_session`。
///
/// 参数:
/// - `context`: 网格上下文
/// - `target`: 目标地址
///
/// 返回:
/// - 允许时返回 Ok，跨会话且未授权时报错
pub(super) fn authorize_target(context: &MeshContext, target: &MeshAddress) -> Result<()> {
    if target.is_local(&context.owner_key, &context.session_id) || context.cross_session {
        return Ok(());
    }
    bail!(
        "mesh target {} is outside this session; set mesh.cross_session=true to allow cross-session messaging",
        target.wire()
    )
}

/// 读取必填的字符串参数。
///
/// 参数:
/// - `args`: 工具参数
/// - `key`: 参数名
///
/// 返回:
/// - 去空白后的字符串；缺失或空白时报错
pub(super) fn required_string_arg(args: &Value, key: &str) -> Result<String> {
    let value = args
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("missing required argument: {key}"))?;
    Ok(value.to_string())
}

/// 读取并校验作用域参数。
///
/// 参数:
/// - `args`: 工具参数
/// - `allowed`: 允许的作用域
/// - `default`: 缺省作用域
///
/// 返回:
/// - 归一化后的作用域
fn scope_arg(args: &Value, allowed: &[&str], default: &str) -> Result<String> {
    let scope = args
        .get("scope")
        .and_then(Value::as_str)
        .unwrap_or(default)
        .trim()
        .to_ascii_lowercase();
    if !allowed.contains(&scope.as_str()) {
        bail!(
            "unsupported scope: {scope} (allowed: {})",
            allowed.join(", ")
        );
    }
    Ok(scope)
}

/// 列出作用域覆盖的会话。
///
/// 参数:
/// - `context`: 网格探测上下文
/// - `scope`: `self` / `workspace` / `all`
///
/// 返回:
/// - 作用域内的会话及其状态目录
fn sessions_in_scope(context: &MeshContext, scope: &str) -> Result<Vec<LocatedSession>> {
    let sessions = crate::state::list_all_sessions(&context.paths)?;
    Ok(match scope {
        "self" => sessions
            .into_iter()
            .filter(|session| Path::new(&context.owner_key) == session.state_dir.as_path())
            .collect(),
        "workspace" => {
            // 会话是按规范化后的路径分工作区存放的，这里必须走同一条规范化，
            // 直接哈希 cwd 会在 Windows 上算出另一个 ID（短名/大小写/UNC 前缀）
            let workspace_id = crate::state::current_workspace_id()?;
            sessions
                .into_iter()
                .filter(|session| session.workspace_id == workspace_id)
                .collect()
        }
        _ => sessions,
    })
}

/// 读取可选的非空字符串参数。
///
/// 参数:
/// - `args`: 工具参数
/// - `key`: 参数名
///
/// 返回:
/// - 去空白后的字符串；缺省或空白时返回空
fn optional_string_arg(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// 返回当前 Unix 秒数。
///
/// 参数:
/// - 无
///
/// 返回:
/// - Unix 秒数
fn unix_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

/// 返回当前 Unix 毫秒数。
///
/// 参数:
/// - 无
///
/// 返回:
/// - Unix 毫秒数
fn unix_millis() -> u64 {
    mailbox::unix_millis()
}
