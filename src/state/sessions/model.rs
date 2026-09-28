use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
