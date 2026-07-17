use super::media::MediaBytes;
use super::message::{MediaKind, OutboundMessage};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum QqTargetKind {
    User,
    Group,
}

impl QqTargetKind {
    /// 从命令行文本解析目标类型。
    ///
    /// 参数:
    /// - `value`: 目标类型文本
    ///
    /// 返回:
    /// - QQ 目标类型
    pub(crate) fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "user" | "c2c" | "private" => Ok(Self::User),
            "group" => Ok(Self::Group),
            _ => bail!("unsupported QQ target kind: {value}"),
        }
    }
}

#[derive(Clone)]
pub(crate) struct QqOfficialClient {
    client: reqwest::Client,
    base_url: String,
    authorization: String,
    target_kind: QqTargetKind,
    target_id: String,
}

impl QqOfficialClient {
    /// 创建 QQ 官方机器人客户端。
    ///
    /// 参数:
    /// - `base_url`: QQ OpenAPI 基础地址
    /// - `authorization`: Authorization 请求头完整内容
    /// - `target_kind`: 目标类型
    /// - `target_id`: openid 或 group_openid
    ///
    /// 返回:
    /// - QQ 官方机器人客户端
    pub(crate) fn new(
        base_url: String,
        authorization: String,
        target_kind: QqTargetKind,
        target_id: String,
    ) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            authorization,
            target_kind,
            target_id,
        }
    }

    /// 发送统一出站消息。
    ///
    /// 参数:
    /// - `message`: 出站消息
    /// - `msg_id`: 被动回复关联消息 ID
    ///
    /// 返回:
    /// - 平台响应列表
    pub(crate) async fn send(
        &self,
        message: &OutboundMessage,
        msg_id: Option<&str>,
    ) -> Result<Vec<Value>> {
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
                self.send_message(json!({
                    "content": text,
                    "msg_type": 0,
                    "msg_id": msg_id,
                }))
                .await?,
            );
        }
        for media in &message.media {
            let bytes = MediaBytes::read(&media.path)?;
            let file_info = self.upload_media(&bytes, media.kind).await?;
            let content = message.text.as_deref().unwrap_or_default();
            responses.push(
                self.send_message(json!({
                    "content": content,
                    "msg_type": 7,
                    "media": { "file_info": file_info },
                    "msg_id": msg_id,
                }))
                .await?,
            );
        }
        Ok(responses)
    }

    /// 上传 QQ 富媒体资源。
    ///
    /// 参数:
    /// - `media`: 媒体内容
    /// - `kind`: 媒体类型
    ///
    /// 返回:
    /// - 可用于发送接口的 file_info
    async fn upload_media(&self, media: &MediaBytes, kind: MediaKind) -> Result<String> {
        let url = match self.target_kind {
            QqTargetKind::User => format!("{}/v2/users/{}/files", self.base_url, self.target_id),
            QqTargetKind::Group => {
                format!("{}/v2/groups/{}/files", self.base_url, self.target_id)
            }
        };
        let file_type = match kind {
            MediaKind::Image => 1,
            MediaKind::File => 4,
        };
        let mut payload = json!({
            "file_type": file_type,
            "file_data": media.base64(),
            "srv_send_msg": false
        });
        if kind == MediaKind::File {
            payload["file_name"] = json!(media.filename);
        }
        let value = self.post_json(&url, payload).await?;
        value
            .get("file_info")
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| anyhow::anyhow!("QQ media upload response has no file_info"))
    }

    /// 发送 QQ 消息。
    ///
    /// 参数:
    /// - `payload`: 消息体
    ///
    /// 返回:
    /// - 平台响应
    async fn send_message(&self, payload: Value) -> Result<Value> {
        let url = match self.target_kind {
            QqTargetKind::User => {
                format!("{}/v2/users/{}/messages", self.base_url, self.target_id)
            }
            QqTargetKind::Group => {
                format!("{}/v2/groups/{}/messages", self.base_url, self.target_id)
            }
        };
        self.post_json(&url, payload).await
    }

    /// 发送带 Authorization 的 JSON 请求。
    ///
    /// 参数:
    /// - `url`: 请求地址
    /// - `payload`: 请求 JSON
    ///
    /// 返回:
    /// - 响应 JSON
    async fn post_json(&self, url: &str, payload: Value) -> Result<Value> {
        let response = self
            .client
            .post(url)
            .header("Authorization", &self.authorization)
            .json(&payload)
            .send()
            .await
            .with_context(|| format!("failed to call QQ API: {url}"))?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            bail!("QQ API returned HTTP {status}: {body}");
        }
        serde_json::from_str::<Value>(&body)
            .with_context(|| format!("invalid QQ API response: {body}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_target_kind_aliases() {
        assert_eq!(QqTargetKind::parse("c2c").unwrap(), QqTargetKind::User);
        assert_eq!(QqTargetKind::parse("group").unwrap(), QqTargetKind::Group);
    }
}
