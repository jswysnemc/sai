use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum MediaKind {
    Image,
    File,
}

#[derive(Debug, Clone)]
pub(crate) struct OutboundMedia {
    pub(crate) kind: MediaKind,
    pub(crate) path: PathBuf,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct OutboundMessage {
    pub(crate) text: Option<String>,
    pub(crate) media: Vec<OutboundMedia>,
}

impl OutboundMessage {
    /// 判断消息是否包含可发送内容。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 是否包含文本或媒体
    pub(crate) fn is_empty(&self) -> bool {
        self.text
            .as_deref()
            .map(str::trim)
            .unwrap_or_default()
            .is_empty()
            && self.media.is_empty()
    }
}
