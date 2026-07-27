use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use tokio::sync::oneshot;
use uuid::Uuid;

/// 等待用户处理的工具权限请求。
#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub(crate) struct PermissionRequest {
    pub(crate) id: String,
    pub(crate) session_id: String,
    pub(crate) tool: String,
    pub(crate) arguments: String,
    /// 是否并行自动审核（供 UI 展示状态）
    #[serde(default)]
    pub(crate) auto_audit: bool,
}

/// 允许决定的来源。
#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize, Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PermissionAllowSource {
    /// 人工确认允许
    #[default]
    Human,
    /// LLM 自动审核允许
    AutoAudit,
}

/// 用户对权限请求作出的决定。
#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub(crate) enum PermissionDecision {
    Allow {
        /// 允许来源；缺省为人工
        #[serde(default)]
        source: PermissionAllowSource,
        /// 放行理由，仅自动审核会给出；纯展示用，不回传给模型
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    Deny { reply: Option<String> },
}

impl PermissionDecision {
    /// 构造人工允许一次。
    ///
    /// 返回:
    /// - 人工来源、无理由的允许决定
    pub(crate) fn allow_once() -> Self {
        Self::Allow {
            source: PermissionAllowSource::Human,
            reason: None,
        }
    }

    /// 构造 LLM 自动允许一次。
    ///
    /// 参数:
    /// - `reason`: 审核模型给出的放行理由，空白会被丢弃
    ///
    /// 返回:
    /// - 自动审核来源的允许决定
    pub(crate) fn auto_allow_once(reason: Option<String>) -> Self {
        Self::Allow {
            source: PermissionAllowSource::AutoAudit,
            reason: reason
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
        }
    }

    /// 读取决定附带的说明文本。
    ///
    /// 允许时是自动审核给出的放行理由，拒绝时是回复给模型的原因。
    ///
    /// 返回:
    /// - 非空说明文本，没有时返回 `None`
    pub(crate) fn detail(&self) -> Option<&str> {
        let value = match self {
            Self::Allow { reason, .. } => reason.as_deref(),
            Self::Deny { reply } => reply.as_deref(),
        };
        value.map(str::trim).filter(|value| !value.is_empty())
    }

    /// 是否为允许决定。
    #[allow(dead_code)]
    pub(crate) fn is_allow(&self) -> bool {
        matches!(self, Self::Allow { .. })
    }

    /// 是否为 LLM 自动允许。
    #[allow(dead_code)]
    pub(crate) fn is_auto_allow(&self) -> bool {
        matches!(
            self,
            Self::Allow {
                source: PermissionAllowSource::AutoAudit,
                ..
            }
        )
    }
}

struct PendingPermission {
    request: PermissionRequest,
    sender: oneshot::Sender<PermissionDecision>,
}

/// 返回进程内共享的等待权限表。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 受互斥锁保护的权限请求集合
fn pending() -> &'static Mutex<HashMap<String, PendingPermission>> {
    static PENDING: OnceLock<Mutex<HashMap<String, PendingPermission>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 创建权限请求并等待用户决定。
///
/// 参数:
/// - `session_id`: 请求所属会话标识
/// - `tool`: 待执行工具名称
/// - `arguments`: 工具参数文本
///
/// 返回:
/// - 权限请求及接收最终决定的一次性通道
pub(crate) fn request_permission(
    session_id: &str,
    tool: &str,
    arguments: &str,
) -> (PermissionRequest, oneshot::Receiver<PermissionDecision>) {
    request_permission_with_auto_audit(session_id, tool, arguments, false)
}

/// 创建权限请求并等待用户/自动审核决定。
///
/// 参数:
/// - `session_id`: 请求所属会话标识
/// - `tool`: 待执行工具名称
/// - `arguments`: 工具参数文本
/// - `auto_audit`: 是否并行自动审核
///
/// 返回:
/// - 权限请求及接收最终决定的一次性通道
pub(crate) fn request_permission_with_auto_audit(
    session_id: &str,
    tool: &str,
    arguments: &str,
    auto_audit: bool,
) -> (PermissionRequest, oneshot::Receiver<PermissionDecision>) {
    // 1. 创建带稳定标识的请求和一次性决定通道
    let request = PermissionRequest {
        id: Uuid::new_v4().to_string(),
        session_id: session_id.to_string(),
        tool: tool.to_string(),
        arguments: arguments.to_string(),
        auto_audit,
    };
    let (sender, receiver) = oneshot::channel();
    // 2. 请求进入共享等待表，供 CLI、TUI 或 Web 查询和处理
    pending().lock().unwrap().insert(
        request.id.clone(),
        PendingPermission {
            request: request.clone(),
            sender,
        },
    );
    (request, receiver)
}

/// 提交权限决定并唤醒等待中的工具。
///
/// 参数:
/// - `id`: 权限请求标识
/// - `decision`: 用户决定
///
/// 返回:
/// - 决定是否成功送达
pub(crate) fn decide_permission(id: &str, decision: PermissionDecision) -> Result<()> {
    let Some(pending) = pending().lock().unwrap().remove(id) else {
        bail!("permission request is no longer pending")
    };
    pending
        .sender
        .send(decision)
        .map_err(|_| anyhow::anyhow!("permission requester is no longer running"))
}


/// 判断权限请求是否仍在等待决定。
///
/// 参数:
/// - `id`: 权限请求标识
///
/// 返回:
/// - 仍在 pending 表中则为 true
pub(crate) fn is_permission_pending(id: &str) -> bool {
    pending().lock().unwrap().contains_key(id)
}

/// 返回指定会话当前等待处理的权限请求。
///
/// 参数:
/// - `session_id`: 会话标识
///
/// 返回:
/// - 当前仍在等待的权限请求
pub(crate) fn pending_permissions(session_id: &str) -> Vec<PermissionRequest> {
    pending()
        .lock()
        .unwrap()
        .values()
        .filter(|entry| entry.request.session_id == session_id)
        .map(|entry| entry.request.clone())
        .collect()
}

/// 立即允许指定会话下全部待审权限（用于切换到 YOLO）。
///
/// 参数:
/// - `session_id`: 会话标识
///
/// 返回:
/// - 被放行的请求数量
pub(crate) fn allow_all_pending_for_session(session_id: &str) -> usize {
    let ids = pending_permissions(session_id)
        .into_iter()
        .map(|request| request.id)
        .collect::<Vec<_>>();
    let mut allowed = 0usize;
    for id in ids {
        if decide_permission(&id, PermissionDecision::allow_once()).is_ok() {
            allowed += 1;
        }
    }
    allowed
}

/// 丢弃指定会话下全部待审权限并关闭等待通道。
///
/// ACP 轮次取消时需要以 cancelled 回包，直接关闭通道可让协议适配层
/// 区分用户拒绝与整个轮次已经终止。
///
/// 参数:
/// - `session_id`: 会话标识
///
/// 返回:
/// - 被撤销的请求数量
pub(crate) fn discard_pending_permissions_for_session(session_id: &str) -> usize {
    let ids = pending_permissions(session_id)
        .into_iter()
        .map(|request| request.id)
        .collect::<Vec<_>>();
    let mut map = pending().lock().unwrap();
    ids.into_iter()
        .filter(|id| map.remove(id).is_some())
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证权限请求会持续等待，直至收到显式决定。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[tokio::test]
    async fn permission_request_waits_for_explicit_decision() {
        let (request, receiver) = request_permission("session", "edit_file", "{}");
        assert!(pending_permissions("session")
            .iter()
            .any(|item| item.id == request.id));
        decide_permission(&request.id, PermissionDecision::allow_once()).unwrap();
        assert!(matches!(receiver.await.unwrap(), PermissionDecision::Allow { .. }));
    }
}
