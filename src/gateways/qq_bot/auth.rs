use anyhow::{bail, Context, Result};
use serde::{Deserialize, Deserializer};
use serde_json::json;
use std::time::{Duration, Instant};

const DEFAULT_TOKEN_URL: &str = "https://bots.qq.com/app/getAppAccessToken";
const TOKEN_REFRESH_SKEW: Duration = Duration::from_secs(60);

#[derive(Debug)]
pub(crate) struct QqBotAuthenticator {
    client: reqwest::Client,
    token_url: String,
    app_id: String,
    client_secret: String,
    cached: Option<CachedToken>,
}

#[derive(Debug)]
struct CachedToken {
    access_token: String,
    expires_at: Instant,
}

#[derive(Debug, Deserialize)]
struct AccessTokenResponse {
    access_token: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_u64")]
    expires_in: Option<u64>,
}

impl QqBotAuthenticator {
    /// 创建 QQ 官方机器人认证器。
    ///
    /// 参数:
    /// - `app_id`: QQ 开放平台 AppID
    /// - `client_secret`: QQ 开放平台 AppSecret
    ///
    /// 返回:
    /// - QQ 官方机器人认证器
    pub(crate) fn new(app_id: String, client_secret: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            token_url: DEFAULT_TOKEN_URL.to_string(),
            app_id,
            client_secret,
            cached: None,
        }
    }

    /// 获取 QQ OpenAPI Authorization 请求头。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - Authorization 请求头完整内容
    pub(crate) async fn authorization(&mut self) -> Result<String> {
        Ok(format!("QQBot {}", self.access_token().await?))
    }

    /// 获取 QQ 官方 access token。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - access token
    pub(crate) async fn access_token(&mut self) -> Result<String> {
        if let Some(token) = self.valid_cached_token() {
            return Ok(token.to_string());
        }
        let token = self.fetch_access_token().await?;
        let access_token = token.access_token.clone();
        self.cached = Some(token);
        Ok(access_token)
    }

    /// 读取仍然有效的缓存 token。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - access token
    fn valid_cached_token(&self) -> Option<&str> {
        let cached = self.cached.as_ref()?;
        if Instant::now() + TOKEN_REFRESH_SKEW >= cached.expires_at {
            return None;
        }
        Some(cached.access_token.as_str())
    }

    /// 请求 QQ 官方 access token。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 缓存 token
    async fn fetch_access_token(&self) -> Result<CachedToken> {
        let response = self
            .client
            .post(&self.token_url)
            .json(&json!({
                "appId": self.app_id,
                "clientSecret": self.client_secret,
            }))
            .send()
            .await
            .with_context(|| "failed to request QQ access token")?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            bail!("QQ token API returned HTTP {status}: {body}");
        }
        let parsed = serde_json::from_str::<AccessTokenResponse>(&body)
            .with_context(|| "invalid QQ token response")?;
        let access_token = parsed
            .access_token
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("QQ token response has no access_token"))?;
        let expires_in = parsed.expires_in.unwrap_or(7200).max(120);
        Ok(CachedToken {
            access_token,
            expires_at: Instant::now() + Duration::from_secs(expires_in),
        })
    }
}

/// 兼容 QQ token 响应中的数字或字符串过期时间。
///
/// 参数:
/// - `deserializer`: Serde 反序列化器
///
/// 返回:
/// - 可选秒数
fn deserialize_optional_u64<'de, D>(deserializer: D) -> std::result::Result<Option<u64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    let Some(value) = value else {
        return Ok(None);
    };
    match value {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::Number(number) => number
            .as_u64()
            .ok_or_else(|| serde::de::Error::custom("expires_in must be a non-negative integer"))
            .map(Some),
        serde_json::Value::String(text) => {
            let text = text.trim();
            if text.is_empty() {
                return Ok(None);
            }
            text.parse::<u64>()
                .map(Some)
                .map_err(|err| serde::de::Error::custom(format!("invalid expires_in: {err}")))
        }
        _ => Err(serde::de::Error::custom(
            "expires_in must be a number or string",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_string_expires_in() {
        let response = serde_json::from_str::<AccessTokenResponse>(
            r#"{"access_token":"token","expires_in":"4789"}"#,
        )
        .unwrap();

        assert_eq!(response.expires_in, Some(4789));
    }

    #[test]
    fn parses_numeric_expires_in() {
        let response = serde_json::from_str::<AccessTokenResponse>(
            r#"{"access_token":"token","expires_in":4789}"#,
        )
        .unwrap();

        assert_eq!(response.expires_in, Some(4789));
    }
}
