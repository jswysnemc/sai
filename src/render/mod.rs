mod asset_block;
mod background_command_event;
mod code_block;
mod command_output;
mod command_result_block;
pub(crate) mod fold_text;
mod cli_command_preview;
mod edit_diff;
mod error;
mod live_tool_status;
mod markdown;
mod markdown_blocks;
mod markdown_inline;
mod permission;
pub(crate) mod session_summary;
#[cfg(test)]
mod session_summary_tests;
mod status_style;
mod stream;
mod stream_config;
mod stream_cursor;
mod stream_output;
mod stream_summary;
mod stream_text;
mod stream_tool_status;
mod streaming_asset_block;
mod streaming_replace;
mod style;
mod table;
pub(crate) mod terminal_image;
mod tool_call_blocks;
mod tool_call_preview;
mod tool_event_line;
mod tool_names;
mod tool_view;
pub(crate) mod transcript;
mod wait_spinner;
pub(crate) mod work_status;

pub(crate) use error::write_chat_error;
pub(crate) use permission::{
    render_auto_audit_status, render_permission_controls, render_permission_decision, render_permission_title,
    PermissionChoice,
};
pub use session_summary::print_session_summary;
pub use stream::StreamRenderer;
pub use stream_config::{ReasoningDisplayMode, StreamRenderOptions, ToolCallDisplayMode};
pub use stream_output::print_assistant_response;
pub(crate) use streaming_replace::rendered_visual_rows;

/// 终端聊天渲染统一使用英文文案。
///
/// 参数:
/// - `english`: 英文文本
/// - `_localized`: 兼容现有调用的本地化文本
///
/// 返回:
/// - 英文文本
pub(crate) const fn terminal_text<'a>(english: &'a str, _localized: &'a str) -> &'a str {
    english
}

/// 渲染直接 CLI 工具调用使用的既有工具视图。
///
/// 参数:
/// - `name`: 工具名称
/// - `arguments`: 工具参数
/// - `mode`: 工具展示模式
///
/// 返回:
/// - diff、命令或普通工具视图文本
pub(crate) fn render_tool_call(name: &str, arguments: &str, mode: ToolCallDisplayMode) -> String {
    if stream_text::is_file_edit_tool(name) {
        return edit_diff::render_edit_file_diff(arguments)
            .unwrap_or_else(|| tool_view::render_call(name, arguments, mode));
    }
    tool_view::render_call(name, arguments, mode)
}

#[allow(unused_imports)]
pub use stream_output::print_markdown;
