//! 终端语义标记：工具类别与交互指示分开，所有字形固定占一列。

pub(crate) const TOOL: &str = "\u{f0ad}";
/// 执行步骤的分组汇总标记，区别于具体文件和工具条目
pub(crate) const ACTIVITY_GROUP: &str = "\u{f03a}";
pub(crate) const READ: &str = "\u{f15c}";
pub(crate) const EDIT: &str = "\u{f044}";
pub(crate) const ASK: &str = "\u{f059}";
pub(crate) const TODO: &str = "\u{f0ae}";
pub(crate) const TRASH: &str = "\u{f1f8}";
pub(crate) const COMMAND: &str = "\u{f489}";
pub(crate) const SEARCH: &str = "\u{f002}";
pub(crate) const BROWSER: &str = "\u{f0ac}";
pub(crate) const ROUTING: &str = "\u{f0b0}";
pub(crate) const SESSION: &str = "\u{f086}";
pub(crate) const AGENTS: &str = "\u{f0c0}";
pub(crate) const INFO: &str = "\u{f05a}";
pub(crate) const THOUGHT: &str = "\u{f0eb}";
pub(crate) const WORKING: &str = "\u{f110}";
const FOLDER: &str = "\u{f07b}";
const CLOCK: &str = "\u{f017}";
const IMAGE: &str = "\u{f03e}";
const DESKTOP: &str = "\u{f108}";
const MEMORY: &str = "\u{f1c0}";
const SHIELD: &str = "\u{f132}";
const PACKAGE: &str = "\u{f187}";
const NETWORK: &str = "\u{f0e8}";
const CALCULATE: &str = "\u{f1ec}";

/// 【终端】【工具标记】按工具职责选择单色字形，未知扩展仍使用通用工具标记。
/// 参数: `name` 为原始或 MCP 限定工具名；返回: 单列语义标记。
pub(crate) fn tool(name: &str) -> &'static str {
    let name = name.rsplit("__").next().unwrap_or(name);
    match name {
        "read_file" | "read" | "man_read" | "online_man_get_page" => READ,
        "str_replace" | "write_file" | "edit_file" | "replace" => EDIT,
        "ask_question" | "ask" => ASK,
        "todo" => TODO,
        "trash_path" | "delete_file" | "remove_path" | "delete_meme" => TRASH,
        "run_command" | "background_command" | "background_notice" => COMMAND,
        "grep" | "glob" | "find_files" | "search_text" => SEARCH,
        "browser" | "web_fetch" | "web_search" | "man_search" | "online_man_search" => BROWSER,
        "subagent" | "agent_probe" => AGENTS,
        "session_probe" | "session_create" | "session_activate" => SESSION,
        "load" | "request_capability" => ROUTING,
        "check_os_info" | "inspect_issue" => DESKTOP,
        "list_directory" | "create_directory" | "get_current_directory" => FOLDER,
        "get_current_time" | "cron" | "set_alarm" | "list_alarms" | "cancel_alarm" => CLOCK,
        "read_image" | "print_image" | "generate_image" | "search_web_images" | "search_meme"
        | "show_meme" | "add_meme" | "update_meme" => IMAGE,
        "compress_context" | "search_evicted_context" => MEMORY,
        "calculate"
        | "calculator"
        | "scientific_calculator"
        | "calculate_hash"
        | "decode_encoded_text" => CALCULATE,
        name if name.starts_with("computer_") => DESKTOP,
        name if name.contains("memory") || name.contains("knowledge_base") => MEMORY,
        name if name.starts_with("ssh_")
            || name.starts_with("mesh_")
            || name.starts_with("mcp_") =>
        {
            NETWORK
        }
        name if name.starts_with("sandbox_") || name.starts_with("permission_") => SHIELD,
        name if name.starts_with("aur_")
            || name.starts_with("pacman_")
            || name.contains("package")
            || name.contains("pkgbuild") =>
        {
            PACKAGE
        }
        name if name.starts_with("read_") => READ,
        name if name.starts_with("search_") || name.ends_with("_query") => SEARCH,
        _ => TOOL,
    }
}

/// 【终端】【语义识别】区分单列图标与正文字符，不依赖标题文案。
/// 参数: `character` 为可见字符；返回: 是否属于本模块图标集合。
pub(crate) fn is_semantic(character: char) -> bool {
    [
        TOOL,
        READ,
        EDIT,
        ASK,
        TODO,
        TRASH,
        COMMAND,
        SEARCH,
        BROWSER,
        ROUTING,
        SESSION,
        AGENTS,
        INFO,
        THOUGHT,
        WORKING,
        FOLDER,
        CLOCK,
        IMAGE,
        DESKTOP,
        MEMORY,
        SHIELD,
        PACKAGE,
        NETWORK,
        CALCULATE,
        ACTIVITY_GROUP,
    ]
    .iter()
    .any(|icon| icon.starts_with(character))
}

/// 【终端】【标题替换】仅替换首个标题引导符，不改动颜色、缩进或后续正文。
/// 参数: `text` 为 ANSI 文本，`icon` 为目标单列标记；返回: 完整渲染文本。
pub(crate) fn replace(text: &str, icon: &str) -> String {
    let mut index = 0;
    while index < text.len() {
        let ch = text[index..].chars().next().unwrap();
        if ch == '\x1b' {
            index = super::terminal_image::escape_sequence_end(text, index).max(index + 1);
        } else if ch.is_whitespace() {
            index += ch.len_utf8();
        } else if is_semantic(ch)
            || matches!(
                ch,
                '●' | '•'
                    | '·'
                    | '○'
                    | '◦'
                    | '◇'
                    | '◈'
                    | '◆'
                    | '▤'
                    | '▸'
                    | '▾'
                    | '›'
                    | '⌄'
                    | '$'
                    | '─'
                    | '╶'
                    | '╴'
                    | '✗'
                    | '❯'
                    | '>'
            )
        {
            return format!("{}{icon}{}", &text[..index], &text[index + ch.len_utf8()..]);
        } else {
            break;
        }
    }
    text.to_string()
}

/// 【终端】【工具动效】仅改变图标亮度，工具语义和正文颜色保持不变。
/// 参数: `icon` 为单列图标，`frame` 为动画帧；返回: 不移动标题的 ANSI 图标。
pub(crate) fn pulse(icon: &str, frame: usize) -> String {
    if (frame / 5) % 2 == 0 {
        format!("\x1b[2m{icon}\x1b[22m")
    } else {
        icon.to_string()
    }
}

/// 【终端】【语义提示】为路由和会话等消息保留独立图标，不借用折叠箭头。
/// 参数: `icon` 为图标，`text` 为提示原文；返回: 带中性样式的提示。
pub(crate) fn notice(icon: &str, text: &str) -> String {
    format!("\x1b[2m{icon} {text}\x1b[0m")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【终端】【工具入口回归】内建类别在运行、成功、失败及不同详略模式中保留语义图标。
    /// 参数: 无；返回: 无。
    #[test]
    fn builtin_render_paths_keep_semantic_icons() {
        use crate::render::{
            tool_view::{render, ToolView},
            ToolCallDisplayMode,
        };
        for name in [
            "read_file",
            "write_file",
            "str_replace",
            "ask_question",
            "todo",
            "trash_path",
            "run_command",
            "background_command",
            "glob",
            "grep",
            "web_search",
            "web_fetch",
            "browser",
            "computer_screenshot",
            "computer_click",
            "generate_image",
            "load",
            "request_capability",
            "compress_context",
            "read_memory",
            "write_memory",
            "mcp_manager",
            "ssh_run_command",
            "ssh_upload_file",
            "agent_probe",
            "session_probe",
            "session_create",
            "mesh_send",
        ] {
            assert_ne!(tool(name), TOOL, "missing semantic mapping: {name}");
            for result in [None, Some(true), Some(false)] {
                let mut view = ToolView::running(name.into(), "{}".into());
                if let Some(ok) = result {
                    view.finish(ok, "fixture result".into());
                }
                for mode in [ToolCallDisplayMode::Summary, ToolCallDisplayMode::Full] {
                    let text = crate::render::activity_animation::strip_ansi_for_test(&render(
                        &view, mode,
                    ));
                    assert!(text.trim_start().starts_with(tool(name)), "{name}: {text}");
                }
            }
        }
    }
    /// 【终端】【图标测试】核心工具标记不同且均占一列，正文替换不误伤；无参数和返回值。
    #[test]
    fn semantic_icons_are_distinct_single_column_and_preserve_body() {
        let icons = [
            READ,
            EDIT,
            ASK,
            TODO,
            TRASH,
            COMMAND,
            SEARCH,
            BROWSER,
            ROUTING,
            SESSION,
            ACTIVITY_GROUP,
        ];
        assert_eq!(
            icons.iter().collect::<std::collections::HashSet<_>>().len(),
            icons.len()
        );
        for icon in icons {
            assert_eq!(unicode_width::UnicodeWidthStr::width(icon), 1);
            assert!(is_semantic(icon.chars().next().unwrap()));
        }
        assert_eq!(
            replace("  \x1b[32m•\x1b[0m title\n• body", READ),
            format!("  \x1b[32m{READ}\x1b[0m title\n• body")
        );
        assert_eq!(replace("2-5/10 · Resume", ">"), "2-5/10 · Resume");
        assert_ne!(tool("web_search"), tool("grep"));
        assert_ne!(tool("mcp__web_search"), tool("mcp__search_text"));
        assert_eq!(tool("run_command"), "\u{f489}");
    }
}
