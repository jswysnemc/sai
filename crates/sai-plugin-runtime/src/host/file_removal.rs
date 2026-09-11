use serde::{Deserialize, Serialize};

/// 【插件文件】【删除类型】永久删除和系统回收站使用不同目录授权，不允许隐式降级
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FileRemovalKind {
    Permanent,
    Trash,
}

/// 【插件文件】【删除请求】只选择一个目标和固定操作类型，不能覆盖宿主目录或权限
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FileRemovalRequest {
    pub path: String,
    pub kind: FileRemovalKind,
}
