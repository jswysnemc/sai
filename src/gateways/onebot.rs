use super::message::{MediaKind, OutboundMessage};
use crate::i18n::text as t;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::path::Path;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum OneBotTargetKind {
    Private,
    Group,
}

impl OneBotTargetKind {
    /// 从命令行文本解析 OneBot 目标类型。
    ///
    /// 参数:
    /// - `value`: 目标类型文本
    ///
    /// 返回:
    /// - OneBot 目标类型
    pub(crate) fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "private" | "user" | "friend" => Ok(Self::Private),
            "group" => Ok(Self::Group),
            _ => bail!(
                "{}: {value}",
                t("unsupported OneBot target kind", "不支持的 OneBot 目标类型")
            ),
        }
    }
}

pub(crate) struct OneBotClient {
    client: reqwest::Client,
    base_url: String,
    access_token: Option<String>,
    target_kind: OneBotTargetKind,
    target_id: i64,
}

impl OneBotClient {
    /// 创建 OneBot HTTP 客户端。
    ///
    /// 参数:
    /// - `base_url`: OneBot HTTP 基础地址
    /// - `access_token`: 访问令牌
    /// - `target_kind`: 目标类型
    /// - `target_id`: user_id 或 group_id
    ///
    /// 返回:
    /// - OneBot HTTP 客户端
    pub(crate) fn new(
        base_url: String,
        access_token: Option<String>,
        target_kind: OneBotTargetKind,
        target_id: i64,
    ) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            access_token,
            target_kind,
            target_id,
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
        if !message.media.is_empty() {
            for media in &message.media {
                match media.kind {
                    MediaKind::Image => {
                        responses.push(
                            self.send_segments(vec![json!({
                                "type": "image",
                                "data": { "file": local_file_uri(&media.path) }
                            })])
                            .await?,
                        );
                    }
                    MediaKind::File => {
                        responses.push(self.upload_file(&media.path).await?);
                    }
                }
            }
        }
        if let Some(text) = message
            .text
            .as_deref()
            .filter(|text| !text.trim().is_empty())
        {
            responses.push(
                self.send_segments(vec![json!({
                    "type": "text",
                    "data": { "text": text }
                })])
                .await?,
            );
        }
        Ok(responses)
    }

    /// 发送 OneBot 消息段。
    ///
    /// 参数:
    /// - `segments`: OneBot 消息段
    ///
    /// 返回:
    /// - 平台响应
    async fn send_segments(&self, segments: Vec<Value>) -> Result<Value> {
        let (path, id_key) = match self.target_kind {
            OneBotTargetKind::Private => ("send_private_msg", "user_id"),
            OneBotTargetKind::Group => ("send_group_msg", "group_id"),
        };
        self.post_json(
            path,
            json!({
                id_key: self.target_id,
                "message": segments,
            }),
        )
        .await
    }

    /// 上传 OneBot 私聊或群文件。
    ///
    /// 参数:
    /// - `path`: 本地文件路径
    ///
    /// 返回:
    /// - 平台响应
    async fn upload_file(&self, path: &Path) -> Result<Value> {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| "file".to_string());
        let (api, id_key) = match self.target_kind {
            OneBotTargetKind::Private => ("upload_private_file", "user_id"),
            OneBotTargetKind::Group => ("upload_group_file", "group_id"),
        };
        self.post_json(
            api,
            json!({
                id_key: self.target_id,
                "file": path.display().to_string(),
                "name": name,
            }),
        )
        .await
    }

    /// 发送 OneBot JSON 请求。
    ///
    /// 参数:
    /// - `api`: OneBot API 名称
    /// - `payload`: 请求 JSON
    ///
    /// 返回:
    /// - 响应 JSON
    async fn post_json(&self, api: &str, payload: Value) -> Result<Value> {
        let url = format!("{}/{}", self.base_url, api);
        let mut request = self.client.post(&url).json(&payload);
        if let Some(token) = self
            .access_token
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            request = request.bearer_auth(token);
        }
        let response = request.send().await.with_context(|| {
            format!(
                "{}: {api}",
                t("failed to call OneBot API", "OneBot API 调用失败")
            )
        })?;
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            bail!(
                "{} HTTP {status}: {body}",
                t("OneBot API returned", "OneBot API 返回")
            );
        }
        let value = serde_json::from_str::<Value>(&body).with_context(|| {
            format!(
                "{}: {body}",
                t("invalid OneBot API response", "无效的 OneBot API 响应")
            )
        })?;
        ensure_ok(&value)?;
        Ok(value)
    }
}

/// 生成 OneBot 本地文件 URI。
///
/// 参数:
/// - `path`: 本地文件路径
///
/// 返回:
/// - OneBot 文件地址
fn local_file_uri(path: &Path) -> String {
    format!("file://{}", path.display())
}

/// 校验 OneBot 响应状态。
///
/// 参数:
/// - `value`: 响应 JSON
///
/// 返回:
/// - 响应是否成功
fn ensure_ok(value: &Value) -> Result<()> {
    let status = value.get("status").and_then(Value::as_str).unwrap_or("ok");
    let retcode = value.get("retcode").and_then(Value::as_i64).unwrap_or(0);
    if status != "ok" || retcode != 0 {
        bail!("{}: {value}", t("OneBot API error", "OneBot API 错误"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_onebot_target_kind_aliases() {
        assert_eq!(
            OneBotTargetKind::parse("friend").unwrap(),
            OneBotTargetKind::Private
        );
        assert_eq!(
            OneBotTargetKind::parse("group").unwrap(),
            OneBotTargetKind::Group
        );
    }
}
