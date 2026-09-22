use crate::paths::SaiPaths;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

/// 用户自建的会话分组。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SidebarGroup {
    pub id: String,
    pub name: String,
    /// 分组色点，使用设计系统里已有的语义色名
    pub color: String,
    pub session_ids: Vec<String>,
}

/// 跨工作区的侧栏索引。视图偏好不在这里，只放需要换浏览器仍保留的数据。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SidebarIndex {
    #[serde(default)]
    pub pinned: Vec<String>,
    #[serde(default)]
    pub archived: Vec<String>,
    #[serde(default)]
    pub unread: BTreeMap<String, String>,
    #[serde(default)]
    pub groups: Vec<SidebarGroup>,
}

/// 侧栏索引的局部更新。缺省字段表示不改。
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SidebarIndexPatch {
    pub pinned: Option<Vec<String>>,
    pub archived: Option<Vec<String>>,
    pub unread: Option<BTreeMap<String, String>>,
    pub groups: Option<Vec<SidebarGroup>>,
    /// 打开会话时清除未读
    pub clear_unread: Option<String>,
    /// 一轮在未被查看的会话结束时写入未读
    pub mark_unread: Option<String>,
}

/// 读取侧栏索引。文件不存在时返回空索引。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - 侧栏索引
pub fn load_sidebar_index(paths: &SaiPaths) -> Result<SidebarIndex> {
    let path = index_path(paths);
    if !path.exists() {
        return Ok(SidebarIndex::default());
    }
    let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))
}

/// 把局部更新写回侧栏索引。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `patch`: 要合并的字段
///
/// 返回:
/// - 合并后的索引
pub fn patch_sidebar_index(paths: &SaiPaths, patch: SidebarIndexPatch) -> Result<SidebarIndex> {
    let mut index = load_sidebar_index(paths).unwrap_or_default();
    if let Some(pinned) = patch.pinned {
        index.pinned = pinned;
    }
    if let Some(archived) = patch.archived {
        index.archived = archived;
    }
    if let Some(unread) = patch.unread {
        index.unread = unread;
    }
    if let Some(groups) = patch.groups {
        index.groups = groups;
    }
    if let Some(session_id) = patch.clear_unread {
        index.unread.remove(&session_id);
    }
    if let Some(session_id) = patch.mark_unread.filter(|id| !index.unread.contains_key(id)) {
        index.unread.insert(session_id, chrono::Utc::now().to_rfc3339());
    }
    save_sidebar_index(paths, &index)?;
    Ok(index)
}

/// 写入侧栏索引。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `index`: 完整索引
///
/// 返回:
/// - 写入是否成功
fn save_sidebar_index(paths: &SaiPaths, index: &SidebarIndex) -> Result<()> {
    let path = index_path(paths);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(index)?;
    fs::write(path, text)?;
    Ok(())
}

/// 返回侧栏索引文件路径。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - JSON 文件路径
fn index_path(paths: &SaiPaths) -> PathBuf {
    paths.state_dir.join("session-sidebar.json")
}
