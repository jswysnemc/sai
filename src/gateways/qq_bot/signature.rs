use crate::i18n::text as t;
use anyhow::{bail, Context, Result};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier};

/// 根据 Bot Secret 生成 Ed25519 签名密钥。
///
/// 参数:
/// - `bot_secret`: QQ 开放平台 Bot Secret
///
/// 返回:
/// - Ed25519 签名密钥
fn signing_key_from_secret(bot_secret: &str) -> Result<SigningKey> {
    let seed = repeated_seed(bot_secret)?;
    Ok(SigningKey::from_bytes(&seed))
}

/// 生成 QQ 官方要求的 32 字节 seed。
///
/// 参数:
/// - `bot_secret`: QQ 开放平台 Bot Secret
///
/// 返回:
/// - 32 字节 seed
fn repeated_seed(bot_secret: &str) -> Result<[u8; 32]> {
    let source = bot_secret.as_bytes();
    if source.is_empty() {
        bail!(t("QQ bot secret is empty", "QQ 机器人密钥为空"));
    }
    let mut seed = [0_u8; 32];
    for index in 0..seed.len() {
        seed[index] = source[index % source.len()];
    }
    Ok(seed)
}

/// 签名 QQ Webhook 回调地址验证请求。
///
/// 参数:
/// - `bot_secret`: QQ 开放平台 Bot Secret
/// - `event_ts`: 验证请求时间戳
/// - `plain_token`: 验证请求随机字符串
///
/// 返回:
/// - 十六进制签名
pub(crate) fn sign_validation(
    bot_secret: &str,
    event_ts: &str,
    plain_token: &str,
) -> Result<String> {
    let key = signing_key_from_secret(bot_secret)?;
    let message = format!("{event_ts}{plain_token}");
    Ok(hex::encode(key.sign(message.as_bytes()).to_bytes()))
}

/// 验证 QQ Webhook 普通事件签名。
///
/// 参数:
/// - `bot_secret`: QQ 开放平台 Bot Secret
/// - `timestamp`: `X-Signature-Timestamp` 请求头
/// - `body`: 原始 HTTP Body
/// - `signature_hex`: `X-Signature-Ed25519` 请求头
///
/// 返回:
/// - 签名是否有效
pub(crate) fn verify_event_signature(
    bot_secret: &str,
    timestamp: &str,
    body: &[u8],
    signature_hex: &str,
) -> Result<()> {
    let key = signing_key_from_secret(bot_secret)?;
    let signature_bytes = hex::decode(signature_hex.trim()).with_context(|| {
        t(
            "invalid QQ webhook signature hex",
            "无效的 QQ Webhook 十六进制签名",
        )
    })?;
    if signature_bytes.len() != 64 || signature_bytes[63] & 224 != 0 {
        bail!(t(
            "invalid QQ webhook signature shape",
            "无效的 QQ Webhook 签名结构"
        ));
    }
    let signature = Signature::try_from(signature_bytes.as_slice()).with_context(|| {
        t(
            "invalid QQ webhook signature bytes",
            "无效的 QQ Webhook 签名字节",
        )
    })?;
    let mut message = Vec::with_capacity(timestamp.len() + body.len());
    message.extend_from_slice(timestamp.as_bytes());
    message.extend_from_slice(body);
    key.verifying_key()
        .verify(&message, &signature)
        .with_context(|| t("invalid QQ webhook signature", "无效的 QQ Webhook 签名"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signs_validation_payload_with_repeated_secret_seed() {
        let signature = sign_validation("naOC0ocQE3shWLAfffVLB1rhYPG7", "1725442341", "abc")
            .expect("signature");

        assert_eq!(signature.len(), 128);
    }

    #[test]
    fn verifies_signed_event_payload() {
        let secret = "naOC0ocQE3shWLAfffVLB1rhYPG7";
        let timestamp = "1725442341";
        let body = br#"{ "op": 0,"d": {}, "t": "GATEWAY_EVENT_NAME"}"#;
        let key = signing_key_from_secret(secret).unwrap();
        let mut message = Vec::new();
        message.extend_from_slice(timestamp.as_bytes());
        message.extend_from_slice(body);
        let signature = hex::encode(key.sign(&message).to_bytes());

        verify_event_signature(secret, timestamp, body, &signature).unwrap();
    }
}
