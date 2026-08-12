//! SSH 交互式征询通道（秘密输入与高危确认）。
//!
//! 这是一条**独立于** `question`/`permission` 的通道，专门解决"秘密绝不进入模型
//! 上下文"的红线。设计要点：
//!
//! - 广播出去的 [`SecretRequest`] 只描述"需要什么"（主机别名、提示、指纹），
//!   **不含任何秘密**。
//! - 用户输入的秘密经一次性通道（oneshot）直达发起请求的工具处理函数，
//!   不经过任何 `AgentEvent`、transcript 或事件流。
//! - 工具处理函数无法直接发出前端事件，因此通过 `ToolProgress` 携带一个带外
//!   **标记**（marker）触发前端；标记同样只含无秘密的请求元信息。
//!
//! 这样即便复用现成的 `ToolProgress` 通道，明文秘密也不会出现在渲染层、Web 事件流
//! 或模型上下文中。

use anyhow::{bail, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use tokio::sync::oneshot;
use uuid::Uuid;

/// 进度通道中秘密请求标记的前缀。
pub(crate) const SECRET_MARKER_PREFIX: &str = "__sai_ssh_secret__:";
/// 进度通道中秘密请求已结束标记的前缀。
pub(crate) const SECRET_DONE_MARKER_PREFIX: &str = "__sai_ssh_secret_done__:";

/// 交互征询的类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InteractiveKind {
    /// 需要输入私钥口令
    Passphrase,
    /// 需要输入登录密码
    Password,
    /// 需要确认远端主机指纹（首次连接或指纹变更）
    HostKey,
    /// 需要确认执行高危命令
    DangerCommand,
}

/// 一次交互征询请求，只描述需求，绝不携带秘密。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SecretRequest {
    /// 请求标识
    pub(crate) id: String,
    /// 所属会话标识
    pub(crate) session_id: String,
    /// 征询类型
    pub(crate) kind: InteractiveKind,
    /// 目标主机别名（脱敏展示用，不含地址凭据）
    pub(crate) host_label: String,
    /// 面向用户的中文提示
    pub(crate) prompt: String,
    /// 主机指纹，仅在指纹确认时给出，供用户核对
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) fingerprint: Option<String>,
    /// 主机指纹是否已变更（相较 known_hosts），仅指纹确认时有意义
    #[serde(default)]
    pub(crate) changed: bool,
}

/// 用户对交互征询给出的应答。
///
/// `Provided` 携带明文秘密，仅在"前端安全输入 -> 后端工具"这条一次性通道上传输，
/// 绝不写入任何广播事件或模型上下文。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", content = "data", rename_all = "snake_case")]
pub(crate) enum SecretResponse {
    /// 提供了秘密文本（口令/密码）
    Provided(String),
    /// 对确认类请求给出是/否
    Confirmed(bool),
    /// 用户取消
    Cancelled,
}

struct PendingSecret {
    request: SecretRequest,
    sender: oneshot::Sender<SecretResponse>,
}

/// 返回进程内共享的等待秘密表。
fn pending() -> &'static Mutex<HashMap<String, PendingSecret>> {
    static PENDING: OnceLock<Mutex<HashMap<String, PendingSecret>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 创建一次交互征询并返回接收应答的通道。
///
/// 参数:
/// - `session_id`: 会话标识
/// - `kind`: 征询类型
/// - `host_label`: 目标主机别名
/// - `prompt`: 面向用户的中文提示
/// - `fingerprint`: 主机指纹，仅指纹确认时给出
/// - `changed`: 指纹是否已变更
///
/// 返回:
/// - 征询请求与接收应答的一次性通道
pub(crate) fn request_secret(
    session_id: &str,
    kind: InteractiveKind,
    host_label: &str,
    prompt: &str,
    fingerprint: Option<String>,
    changed: bool,
) -> (SecretRequest, oneshot::Receiver<SecretResponse>) {
    let request = SecretRequest {
        id: Uuid::new_v4().to_string(),
        session_id: session_id.to_string(),
        kind,
        host_label: host_label.to_string(),
        prompt: prompt.to_string(),
        fingerprint,
        changed,
    };
    let (sender, receiver) = oneshot::channel();
    pending().lock().unwrap().insert(
        request.id.clone(),
        PendingSecret {
            request: request.clone(),
            sender,
        },
    );
    (request, receiver)
}

/// 提交交互应答并唤醒等待中的工具。
///
/// 参数:
/// - `id`: 请求标识
/// - `response`: 用户应答（可能携带秘密）
///
/// 返回:
/// - 应答是否成功送达
pub(crate) fn submit_secret(id: &str, response: SecretResponse) -> Result<()> {
    let Some(pending) = pending().lock().unwrap().remove(id) else {
        bail!("secret request is no longer pending")
    };
    pending
        .sender
        .send(response)
        .map_err(|_| anyhow::anyhow!("secret requester is no longer running"))
}

/// 判断秘密请求是否仍在等待。
///
/// 参数:
/// - `id`: 请求标识
///
/// 返回:
/// - 仍在等待时为 `true`
pub(crate) fn is_pending(id: &str) -> bool {
    pending().lock().unwrap().contains_key(id)
}

/// 返回指定会话当前等待处理的交互征询。
///
/// 参数:
/// - `session_id`: 会话标识
///
/// 返回:
/// - 当前仍在等待的征询请求（不含秘密）
pub(crate) fn pending_ssh_secrets(session_id: &str) -> Vec<SecretRequest> {
    pending()
        .lock()
        .unwrap()
        .values()
        .filter(|entry| entry.request.session_id == session_id)
        .map(|entry| entry.request.clone())
        .collect()
}

/// 丢弃指定会话下全部等待中的交互征询并关闭通道。
///
/// 轮次取消或会话结束时调用，避免工具永久阻塞在已无人应答的通道上。
///
/// 参数:
/// - `session_id`: 会话标识
///
/// 返回:
/// - 被撤销的请求数量
#[allow(dead_code)]
pub(crate) fn discard_for_session(session_id: &str) -> usize {
    let ids = pending_ssh_secrets(session_id)
        .into_iter()
        .map(|request| request.id)
        .collect::<Vec<_>>();
    let mut map = pending().lock().unwrap();
    ids.into_iter()
        .filter(|id| map.remove(id).is_some())
        .count()
}

/// 将征询请求编码为进度通道中的带外标记。
///
/// 标记只包含无秘密的请求元信息，供前端识别并弹出安全输入界面。
///
/// 参数:
/// - `request`: 征询请求
///
/// 返回:
/// - 可安全放入 `ToolProgress` 的单行标记
pub(crate) fn encode_progress_marker(request: &SecretRequest) -> String {
    let json = serde_json::to_string(request).unwrap_or_default();
    let encoded = base64::engine::general_purpose::STANDARD.encode(json.as_bytes());
    format!("{SECRET_MARKER_PREFIX}{encoded}")
}

/// 从进度消息中解析征询请求。
///
/// 参数:
/// - `message`: 进度消息
///
/// 返回:
/// - 命中标记且解析成功时返回请求，否则返回 `None`
pub(crate) fn decode_progress_marker(message: &str) -> Option<SecretRequest> {
    let encoded = message.strip_prefix(SECRET_MARKER_PREFIX)?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// 生成"征询已结束"的带外标记，供前端收起输入界面。
///
/// 参数:
/// - `id`: 请求标识
///
/// 返回:
/// - 可放入 `ToolProgress` 的结束标记
pub(crate) fn encode_resolved_marker(id: &str) -> String {
    format!("{SECRET_DONE_MARKER_PREFIX}{id}")
}

/// 从进度消息中解析"征询已结束"标记。
///
/// 参数:
/// - `message`: 进度消息
///
/// 返回:
/// - 命中结束标记时返回请求标识
pub(crate) fn decode_resolved_marker(message: &str) -> Option<&str> {
    message.strip_prefix(SECRET_DONE_MARKER_PREFIX)
}

/// 判断进度消息是否为本模块的带外标记（请求或结束）。
///
/// 上层据此在把进度转发给渲染层之前拦截，避免标记文本出现在界面或事件流中。
///
/// 参数:
/// - `message`: 进度消息
///
/// 返回:
/// - 属于秘密交互标记时为 `true`
pub(crate) fn is_secret_marker(message: &str) -> bool {
    message.starts_with(SECRET_MARKER_PREFIX) || message.starts_with(SECRET_DONE_MARKER_PREFIX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn provided_secret_reaches_requester() {
        let (request, receiver) = request_secret(
            "session",
            InteractiveKind::Passphrase,
            "build box",
            "输入私钥口令",
            None,
            false,
        );
        assert!(pending_ssh_secrets("session")
            .iter()
            .any(|item| item.id == request.id));
        submit_secret(&request.id, SecretResponse::Provided("s3cret".to_string())).unwrap();
        assert_eq!(
            receiver.await.unwrap(),
            SecretResponse::Provided("s3cret".to_string())
        );
        // 应答后请求应从等待表移除
        assert!(!is_pending(&request.id));
    }

    #[tokio::test]
    async fn confirmation_round_trips() {
        let (request, receiver) = request_secret(
            "session",
            InteractiveKind::DangerCommand,
            "prod",
            "确认执行 rm -rf",
            None,
            false,
        );
        submit_secret(&request.id, SecretResponse::Confirmed(true)).unwrap();
        assert_eq!(receiver.await.unwrap(), SecretResponse::Confirmed(true));
    }

    #[test]
    fn pending_list_is_scoped_by_session() {
        let (a, _rx_a) =
            request_secret("s-a", InteractiveKind::Password, "a", "p", None, false);
        let (b, _rx_b) =
            request_secret("s-b", InteractiveKind::Password, "b", "p", None, false);
        assert!(pending_ssh_secrets("s-a").iter().any(|r| r.id == a.id));
        assert!(pending_ssh_secrets("s-a").iter().all(|r| r.id != b.id));
        // 清理，避免影响其它测试
        let _ = submit_secret(&a.id, SecretResponse::Cancelled);
        let _ = submit_secret(&b.id, SecretResponse::Cancelled);
    }

    #[test]
    fn marker_round_trips_without_leaking_secrets() {
        let request = SecretRequest {
            id: "req-1".to_string(),
            session_id: "session".to_string(),
            kind: InteractiveKind::HostKey,
            host_label: "build box".to_string(),
            prompt: "确认主机指纹".to_string(),
            fingerprint: Some("SHA256:abcd".to_string()),
            changed: false,
        };
        let marker = encode_progress_marker(&request);
        assert!(is_secret_marker(&marker));
        // 标记本身不得含任何明文秘密字段（结构里本就没有秘密）
        let decoded = decode_progress_marker(&marker).expect("应能解码标记");
        assert_eq!(decoded, request);
    }

    #[test]
    fn resolved_marker_round_trips() {
        let marker = encode_resolved_marker("req-9");
        assert!(is_secret_marker(&marker));
        assert_eq!(decode_resolved_marker(&marker), Some("req-9"));
        assert!(decode_progress_marker(&marker).is_none());
    }

    #[test]
    fn plain_progress_is_not_a_marker() {
        assert!(!is_secret_marker("building project ..."));
        assert!(decode_progress_marker("building").is_none());
    }
}
