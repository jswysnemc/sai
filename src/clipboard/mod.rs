mod payload;
mod prompt;
mod reader;

pub use payload::{ClipboardChatInput, ClipboardPayload};
pub use prompt::apply_clipboard_payload;
pub use reader::read_clipboard_payload;
