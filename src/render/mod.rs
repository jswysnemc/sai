pub(crate) mod activity_animation;
mod asset_block;
mod background_command_event;
pub(crate) mod background_promotion;
pub(crate) mod brand_logo;
mod cli_command_preview;
mod code_block;
mod command_output;
mod command_result_block;
pub(crate) mod content_indent;
mod edit_diff;
mod engine_notice;
mod error;
pub(crate) mod expandable;
pub(crate) mod fold_text;
mod live_tool_status;
mod markdown;
mod markdown_blocks;
mod markdown_inline;
pub(crate) mod omitted_line;
mod permission;
pub(crate) mod render_expand;
pub(crate) mod render_width;
pub(crate) mod session_summary;
#[cfg(test)]
mod session_summary_tests;
pub(crate) mod status_style;
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
pub(crate) mod terminal_frame;
pub(crate) mod terminal_image;
pub(crate) mod terminal_paint;
pub(crate) mod todo_style;
mod tool_call_blocks;
mod tool_event_line;
mod tool_names;
mod tool_view;
pub(crate) mod transcript;
mod wait_spinner;
pub(crate) mod work_status;

pub(crate) use command_result_block::command_result_streams;
pub(crate) use engine_notice::engine_notice;
pub(crate) use error::write_chat_error;
pub(crate) use expandable::render_expandable_body;
pub(crate) use permission::{
    render_auto_audit_status, render_permission_controls, render_permission_decision,
    render_permission_decision_for, render_permission_title, PermissionChoice, PermissionView,
};
pub use session_summary::print_session_summary;
pub use stream::StreamRenderer;
pub use stream_config::{ReasoningDisplayMode, StreamRenderOptions, ToolCallDisplayMode};
pub use stream_output::print_assistant_response;
pub(crate) use streaming_replace::{rendered_visual_rows, terminal_width};

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

/// 按终端显示列数截断纯文本，超长时以 `ellipsis` 收尾（计入总预算）。
///
/// 必须用显示宽度而不是字符数：CJK 与 emoji 一个字符占两列，
/// 按字符数截断会让中文路径、参数撑到接近两倍的列宽，
/// 单行工具状态行被挤爆后看起来像截断坏了，而不是有意省略。
///
/// 只处理纯文本：传入带 ANSI 的文本会把样式序列计入宽度。
///
/// 参数:
/// - `text`: 原始纯文本
/// - `max_width`: 允许占用的最大显示列数
/// - `ellipsis`: 超长时追加的省略标记
///
/// 返回:
/// - 截断后的文本
pub(crate) fn clip_to_width(text: &str, max_width: usize, ellipsis: &str) -> String {
    use unicode_segmentation::UnicodeSegmentation;
    use unicode_width::UnicodeWidthStr;

    if UnicodeWidthStr::width(text) <= max_width {
        return text.to_string();
    }
    let budget = max_width.saturating_sub(UnicodeWidthStr::width(ellipsis));
    let mut width = 0usize;
    let mut kept = String::new();
    for grapheme in text.graphemes(true) {
        let grapheme_width = UnicodeWidthStr::width(grapheme);
        if width.saturating_add(grapheme_width) > budget {
            break;
        }
        kept.push_str(grapheme);
        width = width.saturating_add(grapheme_width);
    }
    format!("{kept}{ellipsis}")
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
    // 编辑类与其它工具同一套 ToolView：Summary 为 Write/Replace +N -M，
    // Full 再挂 diff 正文；不再无条件倾倒整段 Added diff。
    tool_view::render_call(name, arguments, mode)
}

#[allow(unused_imports)]
pub use stream_output::print_markdown;

#[cfg(test)]
mod clip_tests {
    use super::clip_to_width;
    use unicode_width::UnicodeWidthStr;

    /// CJK 按显示列数截断：48 列只能放 24 个汉字。
    #[test]
    fn clip_counts_cjk_as_two_columns() {
        let text = "汉".repeat(40);
        let clipped = clip_to_width(&text, 20, "...");

        assert!(
            UnicodeWidthStr::width(clipped.as_str()) <= 20,
            "截断结果超出列预算：{clipped:?}"
        );
        // 20 列预算扣掉 3 列省略号，剩 17 列 → 8 个汉字
        assert_eq!(clipped, format!("{}...", "汉".repeat(8)));
    }

    /// 未超出预算时不改动文本。
    #[test]
    fn clip_leaves_short_text_untouched() {
        assert_eq!(clip_to_width("abc", 10, "..."), "abc");
        assert_eq!(clip_to_width("中文", 4, "..."), "中文");
    }

    /// 预算太小放不下省略号时也不会无限增长。
    #[test]
    fn clip_degrades_gracefully_with_a_tiny_budget() {
        let clipped = clip_to_width("abcdef", 1, "...");
        assert!(UnicodeWidthStr::width(clipped.as_str()) <= 4);
        assert!(clipped.ends_with("..."));
    }
}
