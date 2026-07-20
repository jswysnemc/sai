use serde::{Deserialize, Serialize};

/// TUI / Web 答复完成通知配置；CLI 不读取此项。
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationConfig {
    /// 是否在完成答复后发送桌面 / 浏览器通知
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 是否播放提示音
    #[serde(default = "default_true")]
    pub sound: bool,
}

fn default_true() -> bool {
    true
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sound: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_config_defaults_enabled_with_sound() {
        let config: NotificationConfig = serde_json::from_str("{}").unwrap();
        assert!(config.enabled);
        assert!(config.sound);
    }
}
