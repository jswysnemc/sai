use super::media::MediaBytes;
use super::message::{MediaKind, OutboundMessage};
use crate::i18n::text as t;
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
            bail!(t("message is empty", "消息为空"));
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
            .context(t(
                "failed to send WeCom webhook message",
                "企业微信 Webhook 消息发送失败",
            ))?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            bail!(
                "{} HTTP {status}: {body}",
                t("WeCom webhook returned", "企业微信 Webhook 返回")
            );
        }
        let value = serde_json::from_str::<Value>(&body).with_context(|| {
            format!(
                "{}: {body}",
                t(
                    "invalid WeCom webhook response",
                    "无效的企业微信 Webhook 响应"
                )
            )
        })?;
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
            .context(t(
                "failed to upload WeCom webhook media",
                "企业微信 Webhook 媒体上传失败",
            ))?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            bail!(
                "{} HTTP {status}: {body}",
                t("WeCom media upload returned", "企业微信媒体上传返回")
            );
        }
        let value = serde_json::from_str::<Value>(&body).with_context(|| {
            format!(
                "{}: {body}",
                t(
                    "invalid WeCom media upload response",
                    "无效的企业微信媒体上传响应"
                )
            )
        })?;
        ensure_ok(&value)?;
        value
            .get("media_id")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!(t(
                    "WeCom media upload response has no media_id",
                    "企业微信媒体上传响应缺少 media_id"
                ))
            })
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
        bail!("{}: {value}", t("WeCom API error", "企业微信 API 错误"));
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
    bail!(t(
        "WeCom webhook URL must contain the key query parameter",
        "企业微信 Webhook URL 必须包含 key 查询参数"
    ));
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
