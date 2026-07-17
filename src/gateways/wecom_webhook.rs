use super::media::MediaBytes;
use super::message::{MediaKind, OutboundMessage};
use anyhow::{bail, Context, Result};
use reqwest::multipart;
use serde_json::{json, Value};

pub(crate) struct WecomWebhookClient {
    client: reqwest::Client,
    webhook_url: String,
}

impl WecomWebhookClient {
    /// 创建企业微信 Webhook 客户端。
    ///
    /// 参数:
    /// - `webhook_url`: 企业微信消息推送 Webhook 完整地址
    ///
    /// 返回:
    /// - 企业微信 Webhook 客户端
    pub(crate) fn new(webhook_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            webhook_url,
        }
    }

    /// 发送统一出站消息。
    ///
    /// 参数:
    /// - `message`: 出站消息
    ///
    /// 返回:
    /// - 平台响应列表
    pub(crate) async fn send(&self, message: &OutboundMessage) -> Result<Vec<Value>> {
        if message.is_empty() {
            bail!("message is empty");
        }
        let mut responses = Vec::new();
        if let Some(text) = message
            .text
            .as_deref()
            .filter(|text| !text.trim().is_empty())
        {
            responses.push(
                self.send_json(json!({
                    "msgtype": "text",
                    "text": { "content": text }
                }))
                .await?,
            );
        }
        for media in &message.media {
            let bytes = MediaBytes::read(&media.path)?;
            match media.kind {
                MediaKind::Image => {
                    responses.push(
                        self.send_json(json!({
                            "msgtype": "image",
                            "image": {
                                "base64": bytes.base64(),
                                "md5": bytes.md5_hex()
                            }
                        }))
                        .await?,
                    );
                }
                MediaKind::File => {
                    let media_id = self.upload_file(&bytes).await?;
                    responses.push(
                        self.send_json(json!({
                            "msgtype": "file",
                            "file": { "media_id": media_id }
                        }))
                        .await?,
                    );
                }
            }
        }
        Ok(responses)
    }

    /// 发送 JSON 消息到企业微信 Webhook。
    ///
    /// 参数:
    /// - `payload`: 消息体
    ///
    /// 返回:
    /// - 企业微信响应
    async fn send_json(&self, payload: Value) -> Result<Value> {
        let response = self
            .client
            .post(&self.webhook_url)
            .json(&payload)
            .send()
            .await
            .context("failed to send WeCom webhook message")?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            bail!("WeCom webhook returned HTTP {status}: {body}");
        }
        let value = serde_json::from_str::<Value>(&body)
            .with_context(|| format!("invalid WeCom webhook response: {body}"))?;
        ensure_ok(&value)?;
        Ok(value)
    }

    /// 上传企业微信 Webhook 文件素材。
    ///
    /// 参数:
    /// - `media`: 文件内容
    ///
    /// 返回:
    /// - 三天内有效的 media_id
    async fn upload_file(&self, media: &MediaBytes) -> Result<String> {
        let key = webhook_key(&self.webhook_url)?;
        let upload_url =
            format!("https://qyapi.weixin.qq.com/cgi-bin/webhook/upload_media?key={key}&type=file");
        let part = multipart::Part::bytes(media.bytes.clone())
            .file_name(media.filename.clone())
            .mime_str(media.content_type())?;
        let form = multipart::Form::new().part("media", part);
        let response = self
            .client
            .post(upload_url)
            .multipart(form)
            .send()
            .await
            .context("failed to upload WeCom webhook media")?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            bail!("WeCom media upload returned HTTP {status}: {body}");
        }
        let value = serde_json::from_str::<Value>(&body)
            .with_context(|| format!("invalid WeCom media upload response: {body}"))?;
        ensure_ok(&value)?;
        value
            .get("media_id")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("WeCom media upload response has no media_id"))
    }
}

/// 校验企业微信响应错误码。
///
/// 参数:
/// - `value`: 响应 JSON
///
/// 返回:
/// - 响应是否表示成功
fn ensure_ok(value: &Value) -> Result<()> {
    let errcode = value.get("errcode").and_then(Value::as_i64).unwrap_or(0);
    if errcode != 0 {
        bail!("WeCom API error: {value}");
    }
    Ok(())
}

/// 从企业微信 Webhook URL 提取 key。
///
/// 参数:
/// - `webhook_url`: Webhook 完整地址
///
/// 返回:
/// - Webhook key
fn webhook_key(webhook_url: &str) -> Result<String> {
    for pair in webhook_url.split('?').nth(1).unwrap_or_default().split('&') {
        let Some((key, value)) = pair.split_once('=') else {
            continue;
        };
        if key == "key" && !value.trim().is_empty() {
            return Ok(value.to_string());
        }
    }
    bail!("WeCom webhook URL must contain key query parameter");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_webhook_key() {
        let key = webhook_key("https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=abc").unwrap();

        assert_eq!(key, "abc");
    }
}
