mod background;
mod background_cleanup;
mod background_notice;
mod browser;
mod capability;
pub(crate) mod command_output_buffer;
mod computer_use;
mod context_compression;
mod conversation_notice;
mod file_search;
mod formatter;
mod goal;
mod image;
mod loading;
mod mcp_manager;
mod model;
mod path_search;
mod question;
mod read_file;
mod read_image;
mod result_card;
pub(crate) mod structured_payload;
mod subagent;
mod system_info;
mod timing;
mod todo;
mod trash;
mod web_results;

#[cfg(test)]
mod background_notice_tests;
#[cfg(test)]
mod computer_use_tests;
#[cfg(test)]
mod concise_results_tests;
#[cfg(test)]
mod conversation_notice_tests;
#[cfg(test)]
mod result_card_tests;
#[cfg(test)]
mod structured_payload_tests;
#[cfg(test)]
mod tests;

pub(crate) use background_notice::is_external_notice;
pub(crate) use conversation_notice::ConversationNoticeKind;

pub(crate) use background::{background_pager_body, background_task_command};
pub(crate) use capability::render_capability_output;
pub(crate) use context_compression::{compression_progress_label, compression_result_label};
pub(crate) use formatter::edit_stat_status;
pub(crate) use formatter::{render, render_call, render_framed, render_permission, render_result};
pub(crate) use image::is_image_generation_tool;
pub(crate) use model::{PermissionAuditView, ToolView};
pub(crate) use read_file::parse_read_page;
pub(crate) use read_file::render_body as render_read_body;
pub(crate) use read_image::{image_read_path, render_preview as render_read_image_preview};
pub(crate) use subagent::title as subagent_result_title;
pub(crate) use todo::render_todo_output;

/// 【终端】【专用节点】让执行链复用与普通工具区相同的结构化结果展示。
/// 参数: 工具名称；返回: 是否具备紧凑结果视图。
pub(crate) fn has_compact_result_view(name: &str) -> bool {
    matches!(
        name,
        "grep"
            | "search_text"
            | "check_os_info"
            | "trash_path"
            | "background_command"
            | "glob"
            | "find_files"
            | "create_goal"
            | "get_goal"
            | "update_goal"
            | "mcp_manager"
            | "subagent"
            | "load"
            | "request_capability"
            | "web_search"
            | "browser"
            | "compress_context"
    ) || is_image_generation_tool(name)
}

#[cfg(test)]
mod question_tests;
#[cfg(test)]
mod demo_tests;
#[cfg(test)]
mod comparison_demo_tests;
