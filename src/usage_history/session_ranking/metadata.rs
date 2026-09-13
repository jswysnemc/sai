use crate::paths::SaiPaths;
use crate::usage_history::record::UsageRecord;
use std::collections::HashMap;

/// 会话标题索引，以及旧日志中能够唯一定位的工作区。
pub(super) struct SessionMetadata {
    titles: HashMap<(String, String), String>,
    unique_workspaces: HashMap<String, Option<String>>,
}

impl SessionMetadata {
    /// 【用量统计】【会话定位】只读取会话索引，数据已删除时仍保留用量排行。
    /// 参数: `paths` 为应用路径
    /// 返回: 会话标题及唯一工作区索引
    pub(super) fn load(paths: &SaiPaths) -> Self {
        let mut index = Self {
            titles: HashMap::new(),
            unique_workspaces: HashMap::new(),
        };
        for session in crate::state::list_all_sessions(paths).unwrap_or_default() {
            let id = session.info.id;
            index.titles.insert(
                (session.workspace_id.clone(), id.clone()),
                session.info.title,
            );
            index
                .unique_workspaces
                .entry(id)
                .and_modify(|workspace| {
                    if workspace.as_ref() != Some(&session.workspace_id) {
                        *workspace = None;
                    }
                })
                .or_insert(Some(session.workspace_id));
        }
        index
    }

    /// 【用量统计】【会话定位】优先采用记录中的工作区，旧记录只补充没有歧义的归属。
    /// 参数: `record` 为用量记录，`session_id` 为非空会话标识
    /// 返回: 可确认的工作区标识
    pub(super) fn workspace_id(&self, record: &UsageRecord, session_id: &str) -> Option<String> {
        record
            .workspace_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(str::to_string)
            .or_else(|| self.unique_workspaces.get(session_id).cloned().flatten())
    }

    /// 【用量统计】【会话定位】读取精确匹配的标题，禁止把同名会话链接到其他项目。
    /// 参数: `workspace_id` 为工作区，`session_id` 为会话标识
    /// 返回: 会话仍存在时的标题
    pub(super) fn title(&self, workspace_id: Option<&str>, session_id: &str) -> Option<String> {
        self.titles
            .get(&(workspace_id?.to_string(), session_id.to_string()))
            .cloned()
    }
}
