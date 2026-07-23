use super::event::QqBotValidationEvent;
use crate::i18n::text as t;
use anyhow::{bail, Context, Result};
use axum::http::HeaderMap;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// QQ Webhook 请求允许的最大时钟偏差。
pub(crate) const QQ_WEBHOOK_TIMESTAMP_WINDOW_SECS: u64 = 5 * 60;

const VALIDATION_CACHE_TTL_SECS: u64 = 10 * 60;
const MAX_VALIDATION_CACHE_ENTRIES: usize = 256;
const PLAIN_TOKEN_LENGTH: usize = 20;

#[derive(Debug, Clone)]
struct CachedValidation {
    signature: String,
    seen_at: u64,
}

/// 缓存 QQ 回调地址验证结果，避免同一 challenge 被重复签名。
#[derive(Debug, Default)]
pub(crate) struct ValidationSignatureCache {
    entries: Mutex<HashMap<String, CachedValidation>>,
}

impl ValidationSignatureCache {
    /// 获取或创建一个回调地址验证签名。
    ///
    /// 参数:
    /// - `key`: 由时间戳和 plain_token 组成的 challenge 标识
    /// - `now`: 当前 Unix 时间戳（秒）
    /// - `sign`: 首次遇到 challenge 时生成签名的函数
    ///
    /// 返回:
    /// - 验证签名以及是否命中了缓存
    pub(crate) fn get_or_insert<F>(&self, key: &str, now: u64, sign: F) -> Result<(String, bool)>
    where
        F: FnOnce() -> Result<String>,
    {
        let mut entries = self.entries.lock().map_err(|_| {
            anyhow::anyhow!(t(
                "QQ webhook validation cache is poisoned",
                "QQ Webhook 验证缓存已损坏"
            ))
        })?;
        prune_expired_entries(&mut entries, now);
        if let Some(cached) = entries.get(key) {
            return Ok((cached.signature.clone(), true));
        }

        let signature = sign()?;
        entries.insert(
            key.to_string(),
            CachedValidation {
                signature: signature.clone(),
                seen_at: now,
            },
        );
        trim_cache(&mut entries);
        Ok((signature, false))
    }
}

/// 校验 QQ 回调地址验证事件中的时间戳和随机字符串。
///
/// 参数:
/// - `event`: QQ 回调地址验证事件
/// - `now`: 当前 Unix 时间戳（秒）
///
/// 返回:
/// - 验证是否通过
pub(crate) fn validate_validation_event(event: &QqBotValidationEvent, now: u64) -> Result<()> {
    validate_timestamp(&event.event_ts, now)?;
    validate_plain_token(&event.plain_token)
}

/// 校验 QQ Webhook 普通事件签名使用的时间戳。
///
/// 参数:
/// - `timestamp`: `X-Signature-Timestamp` 请求头
/// - `now`: 当前 Unix 时间戳（秒）
///
/// 返回:
/// - 时间戳是否处于允许窗口内
pub(crate) fn validate_timestamp(timestamp: &str, now: u64) -> Result<()> {
    let value = timestamp.trim();
    if value.is_empty() || value.len() > 12 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        bail!(t(
            "invalid QQ webhook timestamp",
            "无效的 QQ Webhook 时间戳"
        ));
    }
    let timestamp = value
        .parse::<u64>()
        .with_context(|| t("invalid QQ webhook timestamp", "无效的 QQ Webhook 时间戳"))?;
    let difference = timestamp.abs_diff(now);
    if difference > QQ_WEBHOOK_TIMESTAMP_WINDOW_SECS {
        bail!(t(
            "QQ webhook timestamp is outside the allowed window",
            "QQ Webhook 时间戳超出允许时间窗口"
        ));
    }
    Ok(())
}

/// 校验 QQ 回调地址验证请求中的 plain_token。
///
/// 参数:
/// - `plain_token`: QQ 平台提供的随机字符串
///
/// 返回:
/// - plain_token 是否符合协议格式
fn validate_plain_token(plain_token: &str) -> Result<()> {
    if plain_token.len() != PLAIN_TOKEN_LENGTH
        || !plain_token.bytes().all(|byte| byte.is_ascii_alphanumeric())
    {
        bail!(t(
            "invalid QQ webhook plain token",
            "无效的 QQ Webhook plain_token"
        ));
    }
    Ok(())
}

/// 校验 QQ 平台回调地址验证请求的身份标识头。
///
/// 参数:
/// - `headers`: HTTP 请求头
/// - `app_id`: 当前 QQ 机器人 App ID
///
/// 返回:
/// - 请求头是否符合 QQ Webhook 验证请求格式
pub(crate) fn validate_validation_headers(headers: &HeaderMap, app_id: &str) -> Result<()> {
    let received_app_id = headers
        .get("X-Bot-Appid")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!(t("missing X-Bot-Appid", "缺少 X-Bot-Appid")))?;
    if received_app_id != app_id.trim() {
        bail!(t(
            "QQ webhook App ID does not match",
            "QQ Webhook App ID 不匹配"
        ));
    }

    let user_agent = headers
        .get("User-Agent")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!(t(
                "missing QQ webhook user agent",
                "缺少 QQ Webhook User-Agent"
            ))
        })?;
    let product = user_agent.split('/').next().unwrap_or(user_agent).trim();
    if !product.eq_ignore_ascii_case("QQBot-Callback") {
        bail!(t(
            "invalid QQ webhook user agent",
            "无效的 QQ Webhook User-Agent"
        ));
    }
    Ok(())
}

/// 返回当前 Unix 时间戳（秒）。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 当前 Unix 时间戳
pub(crate) fn current_unix_timestamp() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .context(t(
            "system clock is before Unix epoch",
            "系统时钟早于 Unix 纪元",
        ))
}

/// 生成验证 challenge 的缓存键。
///
/// 参数:
/// - `event`: QQ 回调地址验证事件
///
/// 返回:
/// - 稳定的 challenge 缓存键
pub(crate) fn validation_cache_key(event: &QqBotValidationEvent) -> String {
    format!("{}:{}", event.event_ts, event.plain_token)
}

/// 删除过期验证结果。
///
/// 参数:
/// - `entries`: challenge 验证缓存
/// - `now`: 当前 Unix 时间戳
///
/// 返回:
/// - 无
fn prune_expired_entries(entries: &mut HashMap<String, CachedValidation>, now: u64) {
    entries.retain(|_, entry| now.saturating_sub(entry.seen_at) <= VALIDATION_CACHE_TTL_SECS);
}

/// 将验证结果缓存限制在固定容量内。
///
/// 参数:
/// - `entries`: challenge 验证缓存
///
/// 返回:
/// - 无
fn trim_cache(entries: &mut HashMap<String, CachedValidation>) {
    while entries.len() > MAX_VALIDATION_CACHE_ENTRIES {
        let Some(oldest_key) = entries
            .iter()
            .min_by_key(|(_, entry)| entry.seen_at)
            .map(|(key, _)| key.clone())
        else {
            break;
        };
        entries.remove(&oldest_key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gateways::qq_bot::event::QqBotValidationEvent;
    use axum::http::{HeaderMap, HeaderValue};
    use std::cell::Cell;

    /// 验证当前时间窗口内的回调地址验证事件可以通过。
    ///
    /// 返回:
    /// - 无
    #[test]
    fn accepts_current_validation_event() {
        let event = QqBotValidationEvent {
            plain_token: "Arq0D5A61EgUu4OxUvOp".to_string(),
            event_ts: "1_725_442_341".replace('_', ""),
        };
        validate_validation_event(&event, 1_725_442_341).unwrap();
    }

    /// 验证格式错误的 plain_token 会被拒绝。
    ///
    /// 返回:
    /// - 无
    #[test]
    fn rejects_invalid_plain_token_shape() {
        let event = QqBotValidationEvent {
            plain_token: "short".to_string(),
            event_ts: "1725442341".to_string(),
        };
        let error = validate_validation_event(&event, 1_725_442_341).unwrap_err();
        assert!(error.to_string().contains("plain"));
    }

    /// 验证过旧和过早的时间戳都会被拒绝。
    ///
    /// 返回:
    /// - 无
    #[test]
    fn rejects_stale_and_future_timestamps() {
        assert!(validate_timestamp("1725442341", 1_725_442_341 + 301).is_err());
        assert!(validate_timestamp("1725442341", 1_725_442_341 - 301).is_err());
    }

    /// 验证相同 challenge 只会计算一次签名。
    ///
    /// 返回:
    /// - 无
    #[test]
    fn caches_one_signature_per_challenge() {
        let cache = ValidationSignatureCache::default();
        let calls = Cell::new(0);
        let first = cache
            .get_or_insert("key", 100, || {
                calls.set(calls.get() + 1);
                Ok("signature".to_string())
            })
            .unwrap();
        let second = cache
            .get_or_insert("key", 101, || {
                calls.set(calls.get() + 1);
                Ok("different".to_string())
            })
            .unwrap();
        assert_eq!(first, ("signature".to_string(), false));
        assert_eq!(second, ("signature".to_string(), true));
        assert_eq!(calls.get(), 1);
    }

    /// 验证 QQ 回调身份请求头的接受和拒绝路径。
    ///
    /// 返回:
    /// - 无
    #[test]
    fn validates_qq_callback_headers() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Bot-Appid", HeaderValue::from_static("app-1"));
        headers.insert("User-Agent", HeaderValue::from_static("QQBot-Callback"));
        validate_validation_headers(&headers, "app-1").unwrap();
        headers.insert("X-Bot-Appid", HeaderValue::from_static("app-2"));
        assert!(validate_validation_headers(&headers, "app-1").is_err());
    }
}
