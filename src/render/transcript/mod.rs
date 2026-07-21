mod cell;
pub(crate) mod command_output;
mod diff_cell;
mod line;
mod markdown_cell;
mod meta_cell;
pub(crate) mod reasoning_cell;
mod render_cache;
mod shell_cell;
mod store;
mod subagent_cell;
mod tool_cell;
mod user_echo_cell;
mod welcome_cell;
mod window;
mod work_status_cell;

#[cfg(test)]
mod tests;

pub(crate) use cell::TranscriptMode;
pub(crate) use command_output::ExpandableBlock;
pub(crate) use line::AnsiLine;
pub(crate) use store::{TranscriptRenderOptions, TranscriptStore};
pub(crate) use welcome_cell::WelcomeCell;
pub(crate) use window::DisplayWindow;
