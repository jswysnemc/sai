use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use anyhow::{bail, Context, Result};
use base64::Engine;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

type Aes256CbcDecryptor = cbc::Decryptor<aes::Aes256>;

/// 飞书推送过来的一条已解析事件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FeishuInbound {
    /// 事件订阅地址验证，需要原样回 challenge
    UrlVerification { challenge: String },
    /// 收到消息
    Message(FeishuMessage),
    /// 其它事件类型，忽略但不报错
    Ignored,
}

/// 一条飞书消息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FeishuMessage {
    /// 消息所属会话
    pub(crate) chat_id: String,
    /// 消息本身的标识，回复时用于串成话题
    pub(crate) message_id: String,
    /// 发送者的 open_id
    pub(crate) sender_id: String,
    /// 纯文本正文
    pub(crate) text: String,
    /// 是否群聊；群聊里通常要求 @ 机器人才响应
    pub(crate) is_group: bool,
}

/// 事件订阅请求体的外层结构。
#[derive(Deserialize)]
struct EventEnvelope {
    #[serde(default)]
    encrypt: Option<String>,
}

/// 解出明文事件并解析。
///
/// 飞书的事件订阅可开启加密：开启后请求体只有一个 `encrypt` 字段，
/// 内容是 AES-256-CBC 密文，密钥取 encrypt_key 的 SHA-256，IV 是密文前 16 字节。
///
/// 参数:
/// - `body`: 原始请求体
/// - `encrypt_key`: 事件加密密钥；未开启加密时传空串
///
/// 返回:
/// - 解析出的事件
pub(crate) fn parse_inbound(body: &str, encrypt_key: &str) -> Result<FeishuInbound> {
    let envelope = serde_json::from_str::<EventEnvelope>(body)
        .context("failed to parse the Feishu event body")?;
    let plain = match envelope.encrypt {
        Some(cipher) => {
            if encrypt_key.trim().is_empty() {
                bail!("received an encrypted Feishu event but encrypt_key is not configured")
            }
            decrypt_event(&cipher, encrypt_key)?
        }
        None => body.to_string(),
    };
    parse_plain_event(&plain)
}

/// 解密事件密文。
///
/// 参数:
/// - `cipher_base64`: base64 编码的密文
/// - `encrypt_key`: 事件加密密钥
///
/// 返回:
/// - 明文 JSON
fn decrypt_event(cipher_base64: &str, encrypt_key: &str) -> Result<String> {
    let raw = base64::engine::general_purpose::STANDARD
        .decode(cipher_base64.trim())
        .context("Feishu event ciphertext is not valid base64")?;
    if raw.len() <= 16 {
        bail!("Feishu event ciphertext is too short to contain an IV")
    }
    let key = Sha256::digest(encrypt_key.as_bytes());
    let (iv, payload) = raw.split_at(16);
    let decryptor = Aes256CbcDecryptor::new_from_slices(&key, iv)
        .context("failed to build the Feishu event decryptor")?;
    let mut buffer = payload.to_vec();
    let plain = decryptor
        .decrypt_padded_mut::<Pkcs7>(&mut buffer)
        .map_err(|error| anyhow::anyhow!("failed to decrypt the Feishu event: {error}"))?;
    Ok(String::from_utf8_lossy(plain).to_string())
}

/// 解析明文事件。
///
/// 参数:
/// - `plain`: 明文 JSON
///
/// 返回:
/// - 解析出的事件
fn parse_plain_event(plain: &str) -> Result<FeishuInbound> {
    let value = serde_json::from_str::<Value>(plain)
        .context("failed to parse the decrypted Feishu event")?;
    // 1. 地址验证：开放平台配置回调地址时先打一次
    if value.get("type").and_then(Value::as_str) == Some("url_verification") {
        let challenge = value
            .get("challenge")
            .and_then(Value::as_str)
            .context("url_verification event has no challenge")?;
        return Ok(FeishuInbound::UrlVerification {
            challenge: challenge.to_string(),
        });
    }
    // 2. 只处理消息接收事件，其余静默忽略：事件类型很多且会持续增加
    let event_type = value
        .pointer("/header/event_type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if event_type != "im.message.receive_v1" {
        return Ok(FeishuInbound::Ignored);
    }
    let message = value
        .pointer("/event/message")
        .context("message event has no message body")?;
    let chat_id = message
        .get("chat_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let message_id = message
        .get("message_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let is_group = message
        .get("chat_type")
        .and_then(Value::as_str)
        .is_some_and(|kind| kind == "group");
    let sender_id = value
        .pointer("/event/sender/sender_id/open_id")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let text = message_text(message);
    if text.trim().is_empty() {
        // 非文本消息（图片、文件等）暂不处理，忽略而不是报错中断事件流
        return Ok(FeishuInbound::Ignored);
    }
    Ok(FeishuInbound::Message(FeishuMessage {
        chat_id,
        message_id,
        sender_id,
        text,
        is_group,
    }))
}

/// 从消息体取出纯文本。
///
/// 飞书的 `content` 是一段 JSON 字符串，文本消息形如 `{"text":"..."}`。
/// 群聊里 @ 机器人会在正文留下 `@_user_1` 占位，这里一并清掉。
///
/// 参数:
/// - `message`: 消息对象
///
/// 返回:
/// - 纯文本正文
fn message_text(message: &Value) -> String {
    let Some(content) = message.get("content").and_then(Value::as_str) else {
        return String::new();
    };
    let Ok(parsed) = serde_json::from_str::<Value>(content) else {
        return String::new();
    };
    let text = parsed
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or_default();
    strip_mentions(text)
}

/// 清掉正文里的 @ 占位符。
///
/// 参数:
/// - `text`: 原始正文
///
/// 返回:
/// - 去掉占位符并整理空白后的正文
fn strip_mentions(text: &str) -> String {
    let mut cleaned = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("@_user_") {
        cleaned.push_str(&rest[..start]);
        let tail = &rest[start + "@_user_".len()..];
        // 占位符形如 @_user_1，跳过其后的连续数字
        let skip = tail
            .char_indices()
            .find(|(_, ch)| !ch.is_ascii_digit())
            .map(|(index, _)| index)
            .unwrap_or(tail.len());
        rest = &tail[skip..];
    }
    cleaned.push_str(rest);
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_url_verification() {
        let body = r#"{"type":"url_verification","challenge":"abc123"}"#;
        assert_eq!(
            parse_inbound(body, "").unwrap(),
            FeishuInbound::UrlVerification {
                challenge: "abc123".to_string()
            }
        );
    }

    #[test]
    fn parses_a_group_message() {
        let body = serde_json::json!({
            "header": { "event_type": "im.message.receive_v1" },
            "event": {
                "sender": { "sender_id": { "open_id": "ou_1" } },
                "message": {
                    "chat_id": "oc_1",
                    "message_id": "om_1",
                    "chat_type": "group",
                    "content": "{\"text\":\"@_user_1 帮我看下构建\"}"
                }
            }
        })
        .to_string();

        match parse_inbound(&body, "").unwrap() {
            FeishuInbound::Message(message) => {
                assert_eq!(message.chat_id, "oc_1");
                assert_eq!(message.message_id, "om_1");
                assert_eq!(message.sender_id, "ou_1");
                assert!(message.is_group);
                // @ 占位符要清掉，否则会连同占位符一起发给模型
                assert_eq!(message.text, "帮我看下构建");
            }
            other => panic!("expected a message, got {other:?}"),
        }
    }

    /// 非文本消息与其它事件类型都忽略，不能中断事件流。
    #[test]
    fn ignores_non_text_and_other_events() {
        let image = serde_json::json!({
            "header": { "event_type": "im.message.receive_v1" },
            "event": { "message": { "chat_id": "oc_1", "content": "{\"image_key\":\"k\"}" } }
        })
        .to_string();
        assert_eq!(parse_inbound(&image, "").unwrap(), FeishuInbound::Ignored);

        let other = serde_json::json!({
            "header": { "event_type": "im.chat.updated_v1" }
        })
        .to_string();
        assert_eq!(parse_inbound(&other, "").unwrap(), FeishuInbound::Ignored);
    }

    /// 开启加密后收到密文却没配密钥时必须明确报错，而不是当成明文解析失败。
    #[test]
    fn reports_missing_encrypt_key() {
        let body = r#"{"encrypt":"whatever"}"#;
        let error = parse_inbound(body, "").unwrap_err();
        assert!(format!("{error}").contains("encrypt_key"));
    }

    /// 解密路径要能吃下自己加密出来的报文。
    #[test]
    fn decrypts_an_encrypted_event() {
        use aes::cipher::{block_padding::Pkcs7, BlockEncryptMut, KeyIvInit};
        type Aes256CbcEncryptor = cbc::Encryptor<aes::Aes256>;

        let key_source = "test-encrypt-key";
        let plain = r#"{"type":"url_verification","challenge":"xyz"}"#;
        let key = Sha256::digest(key_source.as_bytes());
        let iv = [7u8; 16];
        let encryptor = Aes256CbcEncryptor::new_from_slices(&key, &iv).unwrap();
        let mut buffer = vec![0u8; plain.len() + 16];
        buffer[..plain.len()].copy_from_slice(plain.as_bytes());
        let cipher = encryptor
            .encrypt_padded_mut::<Pkcs7>(&mut buffer, plain.len())
            .unwrap();
        let mut payload = iv.to_vec();
        payload.extend_from_slice(cipher);
        let body = serde_json::json!({
            "encrypt": base64::engine::general_purpose::STANDARD.encode(payload)
        })
        .to_string();

        assert_eq!(
            parse_inbound(&body, key_source).unwrap(),
            FeishuInbound::UrlVerification {
                challenge: "xyz".to_string()
            }
        );
    }
}
