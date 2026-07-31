use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// 提前于实际过期时间刷新的余量。
///
/// 飞书返回的 `expire` 是秒数，贴着过期换取会在网络抖动时命中已失效的令牌。
const TOKEN_REFRESH_MARGIN: Duration = Duration::from_secs(300);

/// 缓存的租户令牌。
struct CachedToken {
    value: String,
    expires_at: Instant,
}

/// 飞书开放平台客户端。
pub(crate) struct FeishuClient {
    http: reqwest::Client,
    base_url: String,
    app_id: String,
    app_secret: String,
    token: Arc<Mutex<Option<CachedToken>>>,
}

impl FeishuClient {
    /// 创建客户端。
    ///
    /// 参数:
    /// - `base_url`: 开放平台接口地址
    /// - `app_id`: 应用 ID
    /// - `app_secret`: 应用密钥
    ///
    /// 返回:
    /// - 客户端
    pub(crate) fn new(base_url: &str, app_id: &str, app_secret: &str) -> Self {
        Self {
            http: reqwest::Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            app_id: app_id.to_string(),
            app_secret: app_secret.to_string(),
            token: Arc::new(Mutex::new(None)),
        }
    }

    /// 取得可用的租户令牌，必要时刷新。
    ///
    /// 返回:
    /// - 租户令牌
    async fn tenant_token(&self) -> Result<String> {
        let mut cached = self.token.lock().await;
        if let Some(token) = cached.as_ref() {
            if Instant::now() < token.expires_at {
                return Ok(token.value.clone());
            }
        }
        let url = format!(
            "{}/open-apis/auth/v3/tenant_access_token/internal",
            self.base_url
        );
        let response = self
            .http
            .post(&url)
            .json(&json!({ "app_id": self.app_id, "app_secret": self.app_secret }))
            .send()
            .await
            .context("failed to request a Feishu tenant token")?
            .json::<Value>()
            .await
            .context("failed to parse the Feishu tenant token response")?;
        ensure_ok(&response, "tenant_access_token")?;
        let value = response
            .get("tenant_access_token")
            .and_then(Value::as_str)
            .context("Feishu response has no tenant_access_token")?
            .to_string();
        let expire = response
            .get("expire")
            .and_then(Value::as_u64)
            .unwrap_or(7200);
        let lifetime = Duration::from_secs(expire).saturating_sub(TOKEN_REFRESH_MARGIN);
        *cached = Some(CachedToken {
            value: value.clone(),
            expires_at: Instant::now() + lifetime,
        });
        Ok(value)
    }

    /// 在指定会话回复一条文本消息。
    ///
    /// 参数:
    /// - `chat_id`: 会话标识
    /// - `text`: 文本正文
    ///
    /// 返回:
    /// - 发送结果
    pub(crate) async fn send_text(&self, chat_id: &str, text: &str) -> Result<()> {
        let token = self.tenant_token().await?;
        let url = format!(
            "{}/open-apis/im/v1/messages?receive_id_type=chat_id",
            self.base_url
        );
        let response = self
            .http
            .post(&url)
            .bearer_auth(token)
            .json(&json!({
                "receive_id": chat_id,
                "msg_type": "text",
                // content 必须是 JSON 字符串而非对象，这是开放平台的约定
                "content": json!({ "text": text }).to_string(),
            }))
            .send()
            .await
            .context("failed to send a Feishu message")?
            .json::<Value>()
            .await
            .context("failed to parse the Feishu send response")?;
        ensure_ok(&response, "send message")
    }
}

/// 校验开放平台返回的业务错误码。
///
/// HTTP 200 不代表成功，飞书把业务错误放在 `code` 字段里。
///
/// 参数:
/// - `response`: 响应体
/// - `action`: 操作名称，用于错误信息
///
/// 返回:
/// - `code` 为 0 时成功
fn ensure_ok(response: &Value, action: &str) -> Result<()> {
    let code = response.get("code").and_then(Value::as_i64).unwrap_or(0);
    if code == 0 {
        return Ok(());
    }
    let message = response
        .get("msg")
        .and_then(Value::as_str)
        .unwrap_or("unknown error");
    bail!("Feishu {action} failed: {message} (code {code})")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_successful_response() {
        assert!(ensure_ok(&json!({ "code": 0 }), "send").is_ok());
        // 缺少 code 字段时按成功处理，部分接口成功响应不带该字段
        assert!(ensure_ok(&json!({}), "send").is_ok());
    }

    /// HTTP 200 不代表业务成功，非零 code 必须报错。
    #[test]
    fn rejects_a_business_error() {
        let error = ensure_ok(
            &json!({ "code": 99991663, "msg": "app ticket invalid" }),
            "send",
        )
        .unwrap_err();
        let text = format!("{error}");
        assert!(text.contains("app ticket invalid"));
        assert!(text.contains("99991663"));
    }
}
