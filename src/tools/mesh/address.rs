use crate::permission::SessionScope;
use anyhow::{bail, Result};
use serde_json::Value;

/// 广播地址字面量。
pub(crate) const BROADCAST: &str = "broadcast";

/// 网格消息地址。
///
/// 三种形态：
/// - `session:<session_id>`：投递给某个会话
/// - `agent:<owner_key>/<agent_id>`：投递给某个会话名下的子智能体
/// - `broadcast`：投递给磁盘上的所有会话
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MeshAddress {
    /// 会话地址，携带会话 id
    Session(String),
    /// 子智能体地址，携带父会话状态目录与子智能体 id
    Agent { owner_key: String, agent_id: String },
    /// 广播，投递给所有会话
    Broadcast,
}

impl MeshAddress {
    /// 解析地址字面量。
    ///
    /// 参数:
    /// - `raw`: 地址字符串
    ///
    /// 返回:
    /// - 解析出的地址；形态不合法时报错
    pub(crate) fn parse(raw: &str) -> Result<Self> {
        let raw = raw.trim();
        if raw.eq_ignore_ascii_case(BROADCAST) {
            return Ok(Self::Broadcast);
        }
        if let Some(session_id) = raw.strip_prefix("session:") {
            let session_id = session_id.trim();
            if session_id.is_empty() {
                bail!("mesh address is missing the session id: {raw}");
            }
            return Ok(Self::Session(session_id.to_string()));
        }
        // owner_key 是绝对路径，本身含 `/`，因此从最后一个 `/` 切开
        if let Some(body) = raw.strip_prefix("agent:") {
            let Some((owner_key, agent_id)) = body.rsplit_once('/') else {
                bail!("mesh agent address must look like agent:<owner_key>/<agent_id>: {raw}");
            };
            let owner_key = owner_key.trim();
            let agent_id = agent_id.trim();
            if owner_key.is_empty() || agent_id.is_empty() {
                bail!("mesh agent address must look like agent:<owner_key>/<agent_id>: {raw}");
            }
            return Ok(Self::Agent {
                owner_key: owner_key.to_string(),
                agent_id: agent_id.to_string(),
            });
        }
        bail!(
            "unsupported mesh address: {raw} (use session:<session_id>, agent:<owner_key>/<agent_id>, or broadcast)"
        );
    }

    /// 序列化为可再次解析的地址字面量。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 地址字符串
    pub(crate) fn wire(&self) -> String {
        match self {
            Self::Session(session_id) => format!("session:{session_id}"),
            Self::Agent {
                owner_key,
                agent_id,
            } => format!("agent:{owner_key}/{agent_id}"),
            Self::Broadcast => BROADCAST.to_string(),
        }
    }

    /// 判断该地址是否属于当前会话。
    ///
    /// 参数:
    /// - `session_key`: 当前会话状态目录（会话唯一身份）
    /// - `session_id`: 当前会话 id
    ///
    /// 返回:
    /// - 属于当前会话时返回 true；广播永远不属于任何单个会话
    pub(crate) fn is_local(&self, session_key: &str, session_id: &str) -> bool {
        match self {
            Self::Session(id) => id == session_id,
            Self::Agent { owner_key, .. } => owner_key == session_key,
            Self::Broadcast => false,
        }
    }

    /// 返回子智能体地址里的子智能体 id。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 子智能体地址返回其 id，其它地址返回 None
    pub(crate) fn agent_id(&self) -> Option<&str> {
        match self {
            Self::Agent { agent_id, .. } => Some(agent_id.as_str()),
            _ => None,
        }
    }

    /// 返回该地址相对当前会话的归属范围。
    ///
    /// 参数:
    /// - `session_key`: 当前会话状态目录
    /// - `session_id`: 当前会话 id
    ///
    /// 返回:
    /// - 属于当前会话为 `Local`，否则为 `CrossSession`
    pub(crate) fn scope(&self, session_key: &str, session_id: &str) -> SessionScope {
        if self.is_local(session_key, session_id) {
            SessionScope::Local
        } else {
            SessionScope::CrossSession
        }
    }
}

/// 判断一次工具调用是否跨越会话边界。
///
/// 供工具注册表在权限判定前分类网格调用；非网格工具一律按 `Local` 处理。
/// `mesh_reply` 的目标要等处理函数解析出原始消息才知道，这里无法判定，
/// 因此按 `Local` 放行，真正的归属校验落在 `mesh_reply` 处理函数里。
///
/// 参数:
/// - `tool`: 工具名称
/// - `arguments`: 工具参数
/// - `session_key`: 当前会话状态目录
/// - `session_id`: 当前会话 id
///
/// 返回:
/// - 本次调用的归属范围
pub(crate) fn session_scope_for_call(
    tool: &str,
    arguments: &Value,
    session_key: &str,
    session_id: &str,
) -> SessionScope {
    if tool != "mesh_send" {
        return SessionScope::Local;
    }
    let Some(target) = arguments.get("to").and_then(Value::as_str) else {
        // 缺参数由工具自己报错，权限层不重复判定
        return SessionScope::Local;
    };
    match MeshAddress::parse(target) {
        Ok(address) => address.scope(session_key, session_id),
        Err(_) => SessionScope::Local,
    }
}
