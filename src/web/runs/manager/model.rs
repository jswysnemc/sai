use super::*;

/// 排队消息插入当前对话的位置。
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum QueueInsertAt {
    /// 当前轮下一次模型请求前插入，续在同一轮
    Request,
    /// 本轮结束后作为新一轮插入
    #[default]
    Turn,
}

/// 启动一轮 Web 对话所需参数。
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct StartRunRequest {
    #[serde(default)]
    pub kind: RunKind,
    pub session_id: String,
    pub input: String,
    // 本地 Agent 选择字段，不会原样进入上游请求。
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub image_url: Option<String>,
    #[serde(default)]
    pub image_urls: Vec<String>,
    #[serde(default)]
    pub mode: Option<String>,
    // provider_id/thinking_level 仅由 resolve_run_config 消费；model 用于本地选择，
    // 解析后的模型名会作为 Chat Completions 协议的 model 字段发送。
    #[serde(default)]
    pub provider_id: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub thinking_level: Option<String>,
    /// 排队时插入位置；立即启动的一轮忽略该字段。
    #[serde(default)]
    pub insert_at: QueueInsertAt,
}

/// Web 运行种类。
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RunKind {
    #[default]
    Conversation,
    Compaction,
    GoalContinuation,
}

/// 活动运行摘要。
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ActiveRunInfo {
    pub run_id: String,
    pub workspace_id: String,
    pub session_id: String,
    pub input: String,
    pub image_urls: Vec<String>,
    pub status: RunCheckpointStatus,
    #[serde(default)]
    pub discard_user_turn: bool,
    #[serde(default)]
    pub restore_input: Option<String>,
    /// 排队插入点；非排队运行保持默认轮次间隔。
    #[serde(default)]
    pub insert_at: QueueInsertAt,
}
