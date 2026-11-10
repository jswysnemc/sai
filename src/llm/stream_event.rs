use super::ChatStreamChunk;

#[derive(Debug, Clone)]
pub enum ChatStreamEvent {
    Reset {
        content_chars: usize,
        reasoning_chars: usize,
    },
    Reconnecting {
        attempt: u32,
        max_attempts: u32,
    },
    /// 【模型流】【调用就绪】完整调用已结束，index 为本响应工具顺序
    ToolCallReady {
        index: usize,
        call: super::ToolCall,
    },
    Chunk(ChatStreamChunk),
    ToolCallProgress(ToolCallStreamProgress),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ToolCallStreamProgress {
    pub index: usize,
    pub name: Option<String>,
    pub arguments_chars: usize,
    pub arguments_bytes: usize,
    pub arguments_preview: String,
    /// 【终端】【行数进度】预览截断时，另带完整参数的新增、删除行数
    pub edit_diff_counts: Option<(usize, usize)>,
}
