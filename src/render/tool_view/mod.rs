mod background;
mod background_notice;
mod capability;
pub(crate) mod command_output_buffer;
mod context_compression;
mod conversation_notice;
mod formatter;
mod image;
mod model;
mod question;
mod read_file;
mod read_image;
mod search;
pub(crate) mod structured_payload;
mod timing;
mod todo;

#[cfg(test)]
mod background_notice_tests;
#[cfg(test)]
mod conversation_notice_tests;
#[cfg(test)]
mod structured_payload_tests;
#[cfg(test)]
mod tests;

pub(crate) use background_notice::is_external_notice;
pub(crate) use conversation_notice::ConversationNoticeKind;

pub(crate) use background::{background_pager_body, background_task_command};
pub(crate) use capability::render_capability_output;
pub(crate) use context_compression::{compression_progress_label, compression_result_label};
pub(crate) use formatter::{render, render_call, render_framed, render_permission, render_result};
pub(crate) use image::is_image_generation_tool;
pub(crate) use model::{PermissionAuditView, ToolView};
pub(crate) use read_file::parse_read_page;
pub(crate) use read_file::render_body as render_read_body;
pub(crate) use read_image::{image_read_path, render_preview as render_read_image_preview};
pub(crate) use todo::render_todo_output;

#[cfg(test)]
mod question_tests;
