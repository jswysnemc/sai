use anyhow::{bail, Context, Result};
use base64::Engine;
use rand::RngCore;
use serde_json::{json, Value};
use std::time::Duration;

const DEFAULT_BOT_AGENT: &str = "Sai/0.1";
const DEFAULT_CDN_BASE_URL: &str = "https://novac2c.cdn.weixin.qq.com/c2c";
const ILINK_APP_ID: &str = "bot";
const ILINK_APP_CLIENT_VERSION: &str = "132102";
const LONG_POLL_TIMEOUT: Duration = Duration::from_secs(40);
const API_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone)]
pub(crate) struct WeixinBotClient {
    client: reqwest::Client,
    base_url: String,
    cdn_base_url: String,
    token: String,
    bot_agent: String,
    verbose: bool,
}

impl WeixinBotClient {
    /// 创建微信官方机器人 iLink 客户端。
    ///
    /// 参数:
    /// - `base_url`: iLink API 基础地址
    /// - `cdn_base_url`: 微信 CDN 基础地址
    /// - `token`: 微信机器人登录 token
    /// - `bot_agent`: 自声明客户端标识
    /// - `verbose`: 是否输出详细日志
    ///
    /// 返回:
    /// - 微信 iLink 客户端
    pub(crate) fn new(
        base_url: String,
        cdn_base_url: String,
        token: String,
        bot_agent: Option<String>,
        verbose: bool,
    ) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            cdn_base_url: cdn_base_url.trim_end_matches('/').to_string(),
            token,
            bot_agent: bot_agent
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| DEFAULT_BOT_AGENT.to_string()),
            verbose,
        }
    }

    /// 长轮询获取微信消息。
    ///
    /// 参数:
    /// - `updates_buf`: 上次响应返回的同步游标
    ///
    /// 返回:
    /// - getUpdates 响应 JSON
    pub(crate) async fn get_updates(&self, updates_buf: Option<&str>) -> Result<Value> {
        self.post_json(
            "ilink/bot/getupdates",
            json!({
                "get_updates_buf": updates_buf.unwrap_or_default(),
                "base_info": self.base_info(),
            }),
            LONG_POLL_TIMEOUT,
        )
        .await
    }

    /// 获取当前微信 iLink API 基础地址。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 当前基础地址
    pub(crate) fn base_url(&self) -> &str {
        &self.base_url
    }

    /// 获取当前微信 CDN 基础地址。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 当前 CDN 基础地址
    pub(crate) fn cdn_base_url(&self) -> &str {
        &self.cdn_base_url
    }

    /// 判断是否开启详细日志。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 是否开启详细日志
    pub(crate) fn verbose(&self) -> bool {
        self.verbose
    }

    /// 输出微信详细调试日志。
    ///
    /// 参数:
    /// - `message`: 日志内容
    ///
    /// 返回:
    /// - 无
    pub(crate) fn debug_log(&self, message: impl AsRef<str>) {
        if self.verbose {
            eprintln!("【微信网关】【调试】{}", message.as_ref());
        }
    }

    /// 获取微信 CDN 上传地址参数。
    ///
    /// 参数:
    /// - `payload`: getuploadurl 请求参数
    ///
    /// 返回:
    /// - getuploadurl 响应 JSON
    pub(crate) async fn get_upload_url(&self, payload: Value) -> Result<Value> {
        let response = self
            .post_json_with_base_info("ilink/bot/getuploadurl", payload, API_TIMEOUT)
            .await?;
        self.debug_log(format!(
            "getuploadurl 响应 upload_full_url={} upload_param={} thumb_upload_param={}",
            response.get("upload_full_url").is_some(),
            response.get("upload_param").is_some(),
            response.get("thumb_upload_param").is_some()
        ));
        Ok(response)
    }

    /// 发送微信文本消息。
    ///
    /// 参数:
    /// - `to_user_id`: 接收方微信 iLink 用户 ID
    /// - `text`: 文本内容
    /// - `context_token`: 入站消息上下文 token
    ///
    /// 返回:
    /// - 发送是否成功
    pub(crate) async fn send_text(
        &self,
        to_user_id: &str,
        text: &str,
        context_token: Option<&str>,
    ) -> Result<()> {
        let client_id = new_client_id();
        let response = self
            .post_json(
                "ilink/bot/sendmessage",
                json!({
                    "msg": {
                        "from_user_id": "",
                        "to_user_id": to_user_id,
                        "client_id": client_id,
                        "message_type": 2,
                        "message_state": 2,
                        "item_list": [
                            { "type": 1, "text_item": { "text": text } }
                        ],
                        "context_token": context_token,
                    },
                    "base_info": self.base_info(),
                }),
                API_TIMEOUT,
            )
            .await?;
        let ret = response.get("ret").and_then(Value::as_i64).unwrap_or(0);
        if ret != 0 {
            bail!("Weixin sendmessage ret={ret}: {response}");
        }
        self.debug_log(format!("sendmessage 文本发送成功 client_id={client_id}"));
        Ok(())
    }

    /// 发送微信结构化消息项。
    ///
    /// 参数:
    /// - `to_user_id`: 接收方微信 iLink 用户 ID
    /// - `item`: 微信 MessageItem JSON
    /// - `context_token`: 入站消息上下文 token
    ///
    /// 返回:
    /// - 发送消息的 client_id
    pub(crate) async fn send_message_item(
        &self,
        to_user_id: &str,
        item: Value,
        context_token: Option<&str>,
    ) -> Result<String> {
        let client_id = new_client_id();
        let response = self
            .post_json(
                "ilink/bot/sendmessage",
                json!({
                    "msg": {
                        "from_user_id": "",
                        "to_user_id": to_user_id,
                        "client_id": client_id,
                        "message_type": 2,
                        "message_state": 2,
                        "item_list": [item],
                        "context_token": context_token,
                    },
                    "base_info": self.base_info(),
                }),
                API_TIMEOUT,
            )
            .await?;
        let ret = response.get("ret").and_then(Value::as_i64).unwrap_or(0);
        if ret != 0 {
            bail!("Weixin sendmessage ret={ret}: {response}");
        }
        self.debug_log(format!("sendmessage 媒体发送成功 client_id={client_id}"));
        Ok(client_id)
    }

    /// 发送携带 base_info 的 iLink JSON 请求。
    ///
    /// 参数:
    /// - `endpoint`: API 路径
    /// - `payload`: 原始业务请求 JSON
    /// - `timeout`: 请求超时
    ///
    /// 返回:
    /// - 响应 JSON
    async fn post_json_with_base_info(
        &self,
        endpoint: &str,
        mut payload: Value,
        timeout: Duration,
    ) -> Result<Value> {
        if let Some(object) = payload.as_object_mut() {
            object.insert("base_info".to_string(), self.base_info());
        }
        self.post_json(endpoint, payload, timeout).await
    }

    /// 发送 iLink JSON 请求。
    ///
    /// 参数:
    /// - `endpoint`: API 路径
    /// - `payload`: 请求 JSON
    /// - `timeout`: 请求超时
    ///
    /// 返回:
    /// - 响应 JSON
    async fn post_json(&self, endpoint: &str, payload: Value, timeout: Duration) -> Result<Value> {
        let url = format!("{}/{}", self.base_url, endpoint);
        self.debug_log(format!(
            "POST {endpoint} timeout={}s payload_bytes={}",
            timeout.as_secs(),
            payload.to_string().len()
        ));
        let response = self
            .client
            .post(&url)
            .headers(self.headers()?)
            .json(&payload)
            .timeout(timeout)
            .send()
            .await
            .with_context(|| format!("failed to call Weixin API: {endpoint}"))?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        self.debug_log(format!(
            "{endpoint} HTTP {status} response_bytes={}",
            body.len()
        ));
        if !status.is_success() {
            bail!("Weixin API returned HTTP {status}: {body}");
        }
        serde_json::from_str::<Value>(&body)
            .with_context(|| format!("invalid Weixin API response: {body}"))
    }

    /// 构建 iLink 请求头。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 请求头集合
    fn headers(&self) -> Result<reqwest::header::HeaderMap> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("Content-Type", "application/json".parse()?);
        headers.insert("AuthorizationType", "ilink_bot_token".parse()?);
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", self.token.trim()).parse()?,
        );
        headers.insert("X-WECHAT-UIN", random_wechat_uin().parse()?);
        headers.insert("iLink-App-Id", ILINK_APP_ID.parse()?);
        headers.insert("iLink-App-ClientVersion", ILINK_APP_CLIENT_VERSION.parse()?);
        Ok(headers)
    }

    /// 构建请求基础信息。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - base_info JSON
    fn base_info(&self) -> Value {
        json!({
            "channel_version": env!("CARGO_PKG_VERSION"),
            "bot_agent": self.bot_agent,
        })
    }
}

/// 生成 X-WECHAT-UIN 请求头。
///
/// 参数:
/// - 无
///
/// 返回:
/// - base64 编码的随机 uint32 字符串
fn random_wechat_uin() -> String {
    let value = rand::thread_rng().next_u32().to_string();
    base64::engine::general_purpose::STANDARD.encode(value)
}

/// 生成微信发送消息 client_id。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 当前消息的唯一客户端 ID
fn new_client_id() -> String {
    format!(
        "sai-{}-{}",
        chrono::Utc::now().timestamp_millis(),
        rand::random::<u16>()
    )
}

/// 返回微信默认 CDN 基础地址。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 微信 CDN 基础地址
pub(crate) fn default_cdn_base_url() -> &'static str {
    DEFAULT_CDN_BASE_URL
}
