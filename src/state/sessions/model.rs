use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DEFAULT_SESSION_ID: &str = "default";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}

/// 跨工作区探测到的一条会话及其状态目录。
#[derive(Debug, Clone)]
pub struct LocatedSession {
    /// 会话索引记录
    pub info: SessionInfo,
    /// 会话所属工作区标识
    pub workspace_id: String,
    /// 会话状态目录；目录本身可能尚未创建
    pub state_dir: PathBuf,
    /// 是否为所在工作区的当前会话
    pub is_current: bool,
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
