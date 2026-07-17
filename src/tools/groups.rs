pub(crate) const BASE_TOOL_NAMES: &[&str] = &[
    "run_command",
    "background_command",
    "subagent",
    "todo",
    "cron",
    "edit_file",
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
        | "deepseek_status" => "web",
        "search_web_images" | "print_image" | "generate_image" | "search_meme" | "show_meme"
        | "add_meme" | "update_meme" | "delete_meme" | "send_channel_image"
        | "send_channel_file" | "send_channel_video" => "media",
        "deep_research"
        | "register_deep_research_topic_title"
        | "register_deep_research_reference"
        | "remove_deep_research_reference" => "research",
        "remember_fact"
        | "search_evicted_context"
        | "recall_past_events"
        | "recall_memory"
        | "recall_memories"
        | "forget_memory"
        | "forget_memories" => "memory",
        "aur_search_packages"
        | "aur_get_package_info"
        | "archlinux_official_package_query"
        | "archlinux_news"
        | "archwiki_query"
        | "man_page_search"
        | "man_page_read"
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
        | "calculate_expression"
        | "draw_zhouyi_hexagram"
        | "draw_tarot_card"
        | "draw_fortune_lot"
        | "roll_dice" => "utilities",
        "set_alarm" | "list_alarms" | "cancel_alarm" => "personal",
        "mcp_manager" => "mcp",
        _ if name.starts_with("mcp_") => "mcp",
        _ if is_base_tool(name) => "base",
        _ => "other",
    }
}

/// 获取用途分组说明。
///
/// 参数:
/// - `group`: 分组名称
///
/// 返回:
/// - 适合展示给模型的分组说明
pub(crate) fn group_description(group: &str) -> &'static str {
    match group {
        "base" => "基础文件、命令和任务操作",
        "web" => "网页搜索、网页读取、天气和在线状态查询",
        "media" => "图片理解、图片生成和表情包操作",
        "research" => "深度研究和引用管理",
        "memory" => "长期记忆、旧上下文和回忆",
        "package" => "Arch Linux、AUR、man 手册和包审查",
        "game" => "Linux 游戏兼容性查询",
        "diagnostics" => "系统诊断和输入法排查",
        "knowledge" => "本地知识库检索和维护",
        "utilities" => "计算、编码、哈希和趣味工具",
        "personal" => "闹钟等个人助手工具",
        "mcp" => "MCP 外部工具服务器",
        _ => "其他工具",
    }
}
