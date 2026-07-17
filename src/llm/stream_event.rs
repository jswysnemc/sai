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
}
