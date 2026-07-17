/// 工具调用状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolCallStatus {
    Pending,
    Completed,
    Error,
    Interrupted,
}

impl ToolCallStatus {
    /// 转换为数据库状态文本。
    ///
    /// 返回:
    /// - 数据库状态文本
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Completed => "completed",
            Self::Error => "error",
            Self::Interrupted => "interrupted",
        }
    }

    /// 从数据库状态文本恢复状态。
    ///
    /// 参数:
    /// - `value`: 数据库状态文本
    ///
    /// 返回:
    /// - 工具调用状态
    pub(crate) fn from_str(value: &str) -> Self {
        match value {
            "completed" => Self::Completed,
            "error" => Self::Error,
            "interrupted" => Self::Interrupted,
            _ => Self::Pending,
        }
    }
}

/// 工具调用记录。
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub struct ToolCallRecord {
    pub id: String,
    pub session_id: String,
    pub turn_id: String,
    pub seq: usize,
    pub provider_call_id: String,
    pub tool_name: String,
    pub arguments: String,
    pub status: ToolCallStatus,
    pub created_at: String,
    pub updated_at: String,
}

/// 工具结果记录。
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub struct ToolResultRecord {
    pub id: String,
    pub session_id: String,
    pub turn_id: String,
    pub provider_call_id: String,
    pub ok: bool,
    pub result_preview: String,
    pub result_ref: Option<String>,
    pub error: Option<String>,
    pub original_chars: usize,
    pub created_at: String,
    pub completed_at: String,
}

/// 工具输出替换记录。
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub struct ToolOutputReplacement {
    pub provider_call_id: String,
    pub session_id: String,
    pub replacement: String,
    pub original_chars: usize,
    pub result_ref: String,
    pub policy: String,
    pub created_at: String,
}

/// 待写入工具调用。
#[derive(Debug, Clone)]
pub(crate) struct NewToolCallRecord {
    pub session_id: String,
    pub turn_id: String,
    pub seq: usize,
    pub provider_call_id: String,
    pub tool_name: String,
    pub arguments: String,
}

/// 待写入工具结果。
#[derive(Debug, Clone)]
pub(crate) struct NewToolResultRecord {
    pub session_id: String,
    pub turn_id: String,
    pub provider_call_id: String,
    pub ok: bool,
    pub result_preview: String,
    pub result_ref: Option<String>,
    pub error: Option<String>,
    pub original_chars: usize,
}

/// 待写入工具输出替换记录。
#[derive(Debug, Clone)]
pub(crate) struct NewToolOutputReplacement {
    pub provider_call_id: String,
    pub session_id: String,
    pub replacement: String,
    pub original_chars: usize,
    pub result_ref: String,
    pub policy: String,
}

/// 工具历史摘要。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolHistorySummary {
    pub call_count: usize,
    pub result_count: usize,
    pub pending_count: usize,
    pub error_count: usize,
    pub replacement_count: usize,
    pub latest_tool_name: Option<String>,
    pub latest_status: Option<ToolCallStatus>,
}

/// 单次工具调用与结果投影记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::state) struct ToolExchangeRecord {
    pub call: ToolCallRecord,
    pub result: Option<ToolResultRecord>,
    pub replacement: Option<ToolOutputReplacement>,
}
