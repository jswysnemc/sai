use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub(super) struct SessionResponse {
    pub(super) id: String,
    pub(super) title: String,
    pub(super) created_at: String,
    pub(super) updated_at: String,
    /// 当前工作区选中的会话指针，不等于终端/网页是否已打开该会话。
    pub(super) active: bool,
    /// 终端或网页已加载该会话（持有者心跳仍存活）。
    pub(super) loaded: bool,
    /// 存活持有者类型：`repl` / `web` / `gateway` 等；未加载时为空。
    pub(super) holder: Option<String>,
}

#[derive(Serialize)]
pub(super) struct WorkspaceSessionsResponse {
    pub(super) workspace_id: String,
    pub(super) workspace_name: String,
    pub(super) workspace_path: String,
    pub(super) last_opened_at: String,
    pub(super) is_git_repository: bool,
    pub(super) active: bool,
    pub(super) sessions: Vec<SessionResponse>,
}

#[derive(Deserialize)]
pub(super) struct CreateSessionRequest {
    pub(super) title: Option<String>,
    pub(super) workspace_id: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct RenameSessionRequest {
    pub(super) title: String,
}

#[derive(Deserialize)]
pub(super) struct BulkDeleteSessionsRequest {
    pub(super) ids: Vec<String>,
}

#[derive(Deserialize)]
pub(super) struct CompactSessionRequest {
    pub(super) provider_id: Option<String>,
    pub(super) model: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct CompactionPolicyRequest {
    pub(super) compaction_ratio: Option<f32>,
    pub(super) compaction_reserve_tokens: Option<usize>,
    pub(super) reset: Option<bool>,
}

#[derive(Serialize)]
pub(super) struct CompactionPolicyResponse {
    pub(super) compaction_ratio: f32,
    pub(super) compaction_reserve_tokens: usize,
    pub(super) compaction_trigger_tokens: usize,
    pub(super) compaction_policy_override: bool,
}

#[derive(Deserialize)]
pub(super) struct RollbackSessionRequest {
    pub(super) turn_id: String,
}

#[derive(Deserialize)]
pub(super) struct HistoryQuery {
    pub(super) limit: Option<usize>,
}

#[derive(Deserialize)]
pub(super) struct SessionEventQuery {
    pub(super) after: Option<u64>,
    /// 会话所属工作区；缺省时使用当前活动工作区
    pub(super) workspace_id: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct ToolResultQuery {
    #[serde(rename = "ref")]
    pub(super) result_ref: String,
}

#[derive(Deserialize)]
pub(super) struct ContextPromptQuery {
    /// 可选 Agent 档案；影响 live 组装路径
    pub(super) agent_id: Option<String>,
    /// 当前供应商；必须与 model 同时提供
    pub(super) provider_id: Option<String>,
    /// 当前模型；必须与 provider_id 同时提供
    pub(super) model: Option<String>,
    /// 当前运行模式
    pub(super) mode: Option<String>,
    /// 界面语言（en / en-US / zh / zh-CN）；缺省跟随服务端环境语言
    pub(super) locale: Option<String>,
}

#[derive(Serialize)]
pub(super) struct DeleteResponse {
    pub(super) deleted: bool,
}

#[derive(Serialize)]
pub(super) struct BulkDeleteResponse {
    pub(super) deleted_ids: Vec<String>,
}

#[derive(Deserialize)]
pub(super) struct RestoreWorktreeRequest {
    pub(super) turn_id: String,
    #[serde(default)]
    pub(super) paths: Vec<String>,
}

#[derive(Serialize)]
pub(super) struct RestoreWorktreeResponse {
    pub(super) restored: bool,
}

#[derive(Serialize)]
pub(super) struct UndoSessionResponse {
    pub(super) removed: usize,
    pub(super) prompt: Option<String>,
    pub(super) worktree_restored: bool,
}

#[derive(Serialize)]
pub(super) struct RollbackSessionResponse {
    pub(super) removed: usize,
    pub(super) prompt: Option<String>,
}

/// 时间线轮次响应：在状态层轮次之上附加本轮使用的模型标识。
#[derive(Serialize)]
pub(super) struct TimelineTurnResponse {
    #[serde(flatten)]
    pub(super) turn: crate::state::SessionTimelineTurn,
    /// 本轮实际使用的模型；历史轮次未记录时缺省
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) model: Option<String>,
}

/// 会话时间线响应：轮次带模型标识，供前端派生模型切换分割线。
#[derive(Serialize)]
pub(super) struct TimelineResponse {
    pub(super) turns: Vec<TimelineTurnResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) compaction: Option<crate::state::SessionTimelineCompaction>,
}
