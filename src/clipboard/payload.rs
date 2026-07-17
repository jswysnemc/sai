#[derive(Debug, Clone)]
pub enum ClipboardPayload {
    Text(String),
    ImageDataUrl {
        data_url: String,
        width: usize,
        height: usize,
    },
}

#[derive(Debug, Clone)]
pub struct ClipboardChatInput {
    pub message: String,
    pub image_url: Option<String>,
}
