use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 浏览器可消费的单条运行事件。
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct WebEvent {
    pub sequence: u64,
    pub run_id: String,
    pub workspace_id: String,
    pub session_id: String,
    pub timestamp: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub payload: Value,
}

impl WebEvent {
    /// 创建尚未分配序号的运行事件。
    ///
    /// 参数:
    /// - `run_id`: 运行 ID
    /// - `workspace_id`: 工作区 ID
    /// - `session_id`: 会话 ID
    /// - `kind`: 事件类型
    /// - `payload`: 事件数据
    ///
    /// 返回:
    /// - Web 运行事件
    pub(crate) fn new(
        run_id: &str,
        workspace_id: &str,
        session_id: &str,
        kind: impl Into<String>,
        payload: Value,
    ) -> Self {
        Self {
            sequence: 0,
            run_id: run_id.to_string(),
            workspace_id: workspace_id.to_string(),
            session_id: session_id.to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            kind: kind.into(),
            payload,
        }
    }
}
