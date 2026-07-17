use serde::{Deserialize, Serialize};

pub const DEFAULT_SESSION_ID: &str = "default";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}

impl SessionInfo {
    /// 创建默认会话信息。
    ///
    /// 参数:
    /// - `now`: 当前时间字符串
    ///
    /// 返回:
    /// - 默认会话信息
    pub fn default_with_time(now: &str) -> Self {
        Self {
            id: DEFAULT_SESSION_ID.to_string(),
            title: "Default".to_string(),
            created_at: now.to_string(),
            updated_at: now.to_string(),
        }
    }
}
