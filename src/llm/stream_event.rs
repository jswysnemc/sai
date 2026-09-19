use super::ChatStreamChunk;

#[derive(Debug, Clone)]
pub enum ChatStreamEvent {
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
