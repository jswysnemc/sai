use serde::{Deserialize, Serialize};

/// Web 服务管理的工作区信息。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct WorkspaceInfo {
    pub id: String,
    pub name: String,
    pub path: String,
    pub last_opened_at: String,
}

/// 持久化的工作区注册表。
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(super) struct WorkspaceRegistry {
    pub active_id: String,
    pub workspaces: Vec<WorkspaceInfo>,
}
