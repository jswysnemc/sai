use super::repl_chrome::{chrome_input_content_cols, ReplChrome};
use super::repl_clipboard::{is_paste_key, paste_image_first, ReplClipboardState};
use super::repl_external_events::ReplExternalEvents;
use super::repl_runtime::{FullscreenEvent, QueuePanelIdleResult, ReplRuntime};
use super::repl_windows_paste::WindowsPasteState;
use super::*;

const EXTERNAL_EVENT_INPUT_POLL_INTERVAL: Duration = Duration::from_millis(50);

mod cleanup;
mod editing;
pub(super) mod event_batch;
mod external_editor;
pub(super) mod history;
pub(super) mod paste;
mod submission;

use cleanup::finish_repl_input;
use editing::*;
#[cfg(test)]
pub(super) use editing::{repl_history_is_clean, repl_should_browse_history};
pub(super) use submission::{ReplInputDraft, ReplInputEvent, ReplInputSubmission};

mod read;
pub(super) use read::read_repl_input;
