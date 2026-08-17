pub(crate) const BASE_TOOL_NAMES: &[&str] = &[
    "run_command",
    "background_command",
    "subagent",
    "todo",
    "cron",
    "edit_file",
    "write_file",
    "str_replace",
    "create_goal",
    "get_goal",
    "update_goal",
    "trash_path",
    "check_os_info",
    "read_file",
    "glob",
    "grep",
    "ask_question",
];

/// 判断工具是否属于基础工具集合。
///
/// 参数:
/// - `name`: 工具名称
///
/// 返回:
/// - 是否为渐进式加载启动时默认暴露的基础工具
pub(crate) fn is_base_tool(name: &str) -> bool {
    BASE_TOOL_NAMES.iter().any(|tool| *tool == name)
}
/// 获取工具所属用途分组。
///
/// 参数:
/// - `name`: 工具名称
///
/// 返回:
/// - 用途分组名称
pub(crate) fn group_for_tool(name: &str) -> &'static str {
    match name {
        "web_search"
        | "web_fetch"
        | "fetch_url"
        | "fcitx5_input_method_wiki_qurey"
        | "query_weather"
        | "get_weather"
        | "convert_exchange_rate"
        | "get_exchange_rate"
        | "exchange_rate"
        | "deepseek_status"
        | "query_deepseek_status" => "web",
        "search_web_images"
        | "print_image"
        | "generate_image"
        | "search_meme"
        | "show_meme"
        | "add_meme"
        | "update_meme"
        | "delete_meme"
        | "send_channel_image"
        | "send_channel_file"
        | "send_channel_video"
        | "send_channel_message" => "media",
        "write_memory"
        | "read_memory"
        | "list_memory"
        | "delete_memory"
        | "search_evicted_context" => "memory",
        "aur_search_packages"
        | "aur_get_package_info"
        | "aur_check_status"
        | "archlinux_official_package_query"
        | "archlinux_news"
        | "archwiki_query"
        | "man_page_search"
        | "man_page_read"
        | "man_search"
        | "man_read"
        | "online_man_search"
        | "online_man_get_page"
        | "review_aur_package"
        | "install_aur_package"
        | "review_pkgbuild_directory" => "package",
        "linux_game_compatibility"
        | "gather_linux_game_compatibility_signals"
        | "register_linux_game_evidence" => "game",
        "deep_diagnose" | "linux_input_method_diagnose" | "check_issue" => "diagnostics",
        "upload_knowledge_base_file"
        | "upload_text_to_knowledge_base"
        | "read_knowledge_base_file"
        | "search_knowledge_base"
        | "search_knowledge_base_by_name"
        | "edit_knowledge_base_file"
        | "remove_knowledge_base_file"
        | "list_knowledge_base_files" => "knowledge",
        "calculate_hash"
        | "decode_encoded_text"
        | "calculate"
        | "calculator"
        | "scientific_calculator"
        | "calculate_expression"
        | "draw_zhouyi_hexagram"
        | "draw_tarot_card"
        | "draw_fortune_lot"
        | "roll_dice" => "utilities",
        "set_alarm" | "list_alarms" | "cancel_alarm" => "personal",
        "ssh_list_hosts" | "ssh_run_command" | "ssh_upload_file" | "ssh_download_file" => "ssh",
        "mcp_manager" => "mcp",
        _ if name.starts_with("mcp_") => "mcp",
        _ if is_base_tool(name) => "base",
        _ => "other",
    }
}

/// 用途分组的展示、排序与交互说明。
#[derive(Clone, Copy)]
pub(crate) struct ToolGroupMeta {
    /// 设置页排序，数字越小越靠前
    pub rank: u8,
    /// 英文短标题
    pub label_en: &'static str,
    /// 中文短标题
    pub label_zh: &'static str,
    /// 英文用户说明；空表示该组无需额外解释
    pub hint_en: &'static str,
    /// 中文用户说明
    pub hint_zh: &'static str,
    /// 给模型看的分组说明（英文，配合 load 提示）
    pub model_description: &'static str,
    /// 相关设置页路径
    pub settings_path: Option<&'static str>,
}

const UNKNOWN_GROUP: ToolGroupMeta = ToolGroupMeta {
    rank: 90,
    label_en: "Other",
    label_zh: "其他",
    hint_en: "",
    hint_zh: "",
    model_description: "Other tools",
    settings_path: None,
};

/// 返回用途分组的展示与排序元数据。
///
/// 参数:
/// - `group`: 分组标识
///
/// 返回:
/// - 该组的标题、提示、模型说明与排序
pub(crate) fn group_meta(group: &str) -> ToolGroupMeta {
    match group {
        "base" => ToolGroupMeta {
            rank: 0,
            label_en: "Base",
            label_zh: "基础操作",
            hint_en: "",
            hint_zh: "",
            model_description: "Core file, command, and task tools",
            settings_path: None,
        },
        "ssh" => ToolGroupMeta {
            rank: 1,
            label_en: "SSH",
            label_zh: "SSH 远程",
            hint_en: "You add hosts and type passwords in Settings → SSH. The model only sees host aliases and command output — never keys or passwords. Dangerous commands still ask you to confirm.",
            hint_zh: "主机和密码在「设置 → SSH」里由你配置和输入。模型只能看到主机别名和命令结果，看不到密钥或密码。高危命令仍会再向你确认一次。",
            model_description: "Remote SSH: list hosts, run commands, transfer files. Call hosts by host_id only; credentials stay in the user UI.",
            settings_path: Some("/settings/ssh"),
        },
        "web" => ToolGroupMeta {
            rank: 2,
            label_en: "Web",
            label_zh: "网页检索",
            hint_en: "",
            hint_zh: "",
            model_description: "Web search, page fetch, weather, and online status",
            settings_path: None,
        },
        "media" => ToolGroupMeta {
            rank: 3,
            label_en: "Media",
            label_zh: "媒体",
            hint_en: "",
            hint_zh: "",
            model_description: "Image understanding, generation, and memes",
            settings_path: None,
        },
        "memory" => ToolGroupMeta {
            rank: 4,
            label_en: "Memory",
            label_zh: "记忆",
            hint_en: "",
            hint_zh: "",
            model_description: "Long-term memory, evicted context, and recall",
            settings_path: None,
        },
        "knowledge" => ToolGroupMeta {
            rank: 5,
            label_en: "Knowledge",
            label_zh: "知识库",
            hint_en: "",
            hint_zh: "",
            model_description: "Local knowledge-base search and maintenance",
            settings_path: None,
        },
        "package" => ToolGroupMeta {
            rank: 6,
            label_en: "Packages",
            label_zh: "软件包",
            hint_en: "",
            hint_zh: "",
            model_description: "Arch Linux, AUR, man pages, and package review",
            settings_path: None,
        },
        "diagnostics" => ToolGroupMeta {
            rank: 7,
            label_en: "Diagnostics",
            label_zh: "诊断",
            hint_en: "",
            hint_zh: "",
            model_description: "System diagnostics and input-method troubleshooting",
            settings_path: None,
        },
        "game" => ToolGroupMeta {
            rank: 8,
            label_en: "Games",
            label_zh: "游戏",
            hint_en: "",
            hint_zh: "",
            model_description: "Linux game compatibility lookup",
            settings_path: None,
        },
        "utilities" => ToolGroupMeta {
            rank: 9,
            label_en: "Utilities",
            label_zh: "实用工具",
            hint_en: "",
            hint_zh: "",
            model_description: "Calculator, encoding, hash, and novelty tools",
            settings_path: None,
        },
        "personal" => ToolGroupMeta {
            rank: 10,
            label_en: "Personal",
            label_zh: "个人",
            hint_en: "",
            hint_zh: "",
            model_description: "Personal-assistant tools such as alarms",
            settings_path: None,
        },
        "mcp" => ToolGroupMeta {
            rank: 11,
            label_en: "MCP",
            label_zh: "MCP",
            hint_en: "",
            hint_zh: "",
            model_description: "External MCP tool servers",
            settings_path: None,
        },
        "other" => UNKNOWN_GROUP,
        _ => UNKNOWN_GROUP,
    }
}

/// 返回分组在设置页中的排序权重。
///
/// 参数:
/// - `group`: 分组标识
///
/// 返回:
/// - 越小越靠前
pub(crate) fn group_rank(group: &str) -> u8 {
    group_meta(group).rank
}

/// 获取用途分组说明。
///
/// 参数:
/// - `group`: 分组名称
///
/// 返回:
/// - 适合展示给模型的分组说明
pub(crate) fn group_description(group: &str) -> &'static str {
    group_meta(group).model_description
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SSH 组必须紧挨基础组，避免按字母序沉到列表底部。
    #[test]
    fn ssh_ranks_immediately_after_base() {
        assert_eq!(group_rank("base"), 0);
        assert_eq!(group_rank("ssh"), 1);
        assert!(group_rank("ssh") < group_rank("web"));
        assert!(group_rank("ssh") < group_rank("utilities"));
    }

    /// SSH 组必须带用户/模型分工说明，并指向主机设置页。
    #[test]
    fn ssh_meta_explains_user_versus_model() {
        let meta = group_meta("ssh");
        assert_eq!(meta.label_zh, "SSH 远程");
        assert_eq!(meta.label_en, "SSH");
        assert!(meta.hint_zh.contains("设置 → SSH"));
        assert!(meta.hint_en.contains("Settings → SSH"));
        assert!(meta.model_description.contains("host_id"));
        assert_eq!(meta.settings_path, Some("/settings/ssh"));
    }
}
