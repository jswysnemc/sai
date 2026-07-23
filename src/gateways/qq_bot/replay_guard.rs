use crate::i18n::text as t;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const MESSAGE_DEDUPE_TTL: Duration = Duration::from_secs(60 * 60);
const MAX_MESSAGE_IDS: usize = 4096;

/// 为入站 QQ 消息提供有界的内存去重保护。
#[derive(Debug)]
pub(crate) struct MessageReplayGuard {
    entries: Mutex<HashMap<String, Instant>>,
    ttl: Duration,
    capacity: usize,
}

impl Default for MessageReplayGuard {
    /// 创建默认的一小时、4096 条消息去重窗口。
    ///
    /// 返回:
    /// - 默认消息去重保护器
    fn default() -> Self {
        Self::new(MESSAGE_DEDUPE_TTL, MAX_MESSAGE_IDS)
    }
}

impl MessageReplayGuard {
    /// 创建消息去重保护器。
    ///
    /// 参数:
    /// - `ttl`: 消息标识保留时间
    /// - `capacity`: 最多保留的消息标识数量
    ///
    /// 返回:
    /// - 消息去重保护器
    pub(crate) fn new(ttl: Duration, capacity: usize) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            ttl,
            capacity: capacity.max(1),
        }
    }

    /// 尝试登记一条尚未处理的消息。
    ///
    /// 参数:
    /// - `message_id`: QQ 消息标识
    ///
    /// 返回:
    /// - `true` 表示首次登记，`false` 表示处于去重窗口内
    pub(crate) fn try_claim(&self, message_id: &str) -> Result<bool> {
        let now = Instant::now();
        let mut entries = self.entries.lock().map_err(|_| {
            anyhow!(t(
                "QQ message replay guard is poisoned",
                "QQ 消息去重保护器已损坏"
            ))
        })?;
        entries.retain(|_, seen_at| now.duration_since(*seen_at) <= self.ttl);
        if entries.contains_key(message_id) {
            return Ok(false);
        }
        entries.insert(message_id.to_string(), now);
        self.trim_oldest(&mut entries);
        Ok(true)
    }

    /// 释放处理失败的消息标识，使平台后续重试可以重新处理。
    ///
    /// 参数:
    /// - `message_id`: QQ 消息标识
    ///
    /// 返回:
    /// - 是否删除了已有登记
    pub(crate) fn release(&self, message_id: &str) -> Result<bool> {
        let mut entries = self.entries.lock().map_err(|_| {
            anyhow!(t(
                "QQ message replay guard is poisoned",
                "QQ 消息去重保护器已损坏"
            ))
        })?;
        Ok(entries.remove(message_id).is_some())
    }

    /// 将去重缓存限制在固定容量内。
    ///
    /// 参数:
    /// - `entries`: 待裁剪的消息标识和登记时间
    ///
    /// 返回:
    /// - 无
    fn trim_oldest(&self, entries: &mut HashMap<String, Instant>) {
        while entries.len() > self.capacity {
            let Some(oldest_key) = entries
                .iter()
                .min_by_key(|(_, seen_at)| **seen_at)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            entries.remove(&oldest_key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证同一消息在释放前只能登记一次。
    ///
    /// 返回:
    /// - 无
    #[test]
    fn claims_message_only_once() {
        let guard = MessageReplayGuard::new(Duration::from_secs(60), 8);
        assert!(guard.try_claim("message-1").unwrap());
        assert!(!guard.try_claim("message-1").unwrap());
        assert!(guard.try_claim("message-2").unwrap());
        assert!(guard.release("message-1").unwrap());
        assert!(guard.try_claim("message-1").unwrap());
    }

    /// 验证消息去重缓存不会超过配置容量。
    ///
    /// 返回:
    /// - 无
    #[test]
    fn bounds_cache_capacity() {
        let guard = MessageReplayGuard::new(Duration::from_secs(60), 2);
        assert!(guard.try_claim("message-1").unwrap());
        assert!(guard.try_claim("message-2").unwrap());
        assert!(guard.try_claim("message-3").unwrap());
        assert_eq!(guard.entries.lock().unwrap().len(), 2);
    }
}
