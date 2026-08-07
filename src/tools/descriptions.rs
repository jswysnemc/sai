use crate::i18n::text as t;

/// 返回内置工具的短用途说明。
///
/// 参数:
/// - `name`: 工具名称
/// - `fallback`: 动态工具或未知工具提供的原始说明
///
/// 返回:
/// - 已知内置工具返回显式短说明，动态工具返回原始说明
pub(crate) fn tool_description(name: &str, fallback: &str) -> String {
    let description = match name {
        "run_command" => t("Run shell commands.", "执行 Shell 命令。"),
        "background_command" => t("Manage background commands.", "管理后台命令。"),
        "subagent" => t("Start or manage subagents.", "启动或管理子智能体。"),
        "todo" => t("Manage the session task list.", "管理会话任务清单。"),
        "cron" => t("Manage scheduled Gateway tasks.", "管理网关定时任务。"),
        "edit_file" => t("Apply patches to files.", "向文件应用补丁。"),
        "write_file" => t("Create or overwrite text files.", "创建或覆盖文本文件。"),
        "str_replace" => t("Replace exact text in a file.", "精确替换文件文本。"),
        "create_goal" => t("Create a persistent goal.", "创建持续目标。"),
        "get_goal" => t("Read the active goal.", "读取当前目标。"),
        "update_goal" => t("Update the active goal.", "更新当前目标。"),
        "trash_path" => t("Move a path to system Trash.", "把路径移入系统回收站。"),
        "check_os_info" => t("Read basic system information.", "读取基础系统信息。"),
        "read_file" => t(
            "Read files, directories, or images.",
            "读取文件、目录或图片。",
        ),
        "glob" | "find_files" => t("Find files by glob pattern.", "按 Glob 模式查找文件。"),
        "grep" | "search_text" => t("Search file contents.", "搜索文件内容。"),
        "ask_question" => t(
            "Ask the user structured questions.",
            "向用户提出结构化问题。",
        ),
        "invoke_tool" => t("Invoke a loaded tool.", "调用已加载工具。"),
        "mcp_manager" => t("Inspect or control MCP servers.", "检查或控制 MCP 服务。"),
        "web_search" => t("Search the web.", "搜索网页。"),
        "web_fetch" | "fetch_url" => t("Read a web page.", "读取网页。"),
        "get_weather" | "weather" | "query_weather" => t("Read weather data.", "查询天气。"),
        "get_exchange_rate" | "exchange_rate" | "convert_exchange_rate" => {
            t("Read currency exchange rates.", "查询货币汇率。")
        }
        "query_deepseek_status" | "deepseek_status" => {
            t("Read DeepSeek service status.", "查询 DeepSeek 服务状态。")
        }
        "fcitx5_input_method_wiki_qurey" => t("Read Fcitx 5 Wiki guidance.", "查询 Fcitx 5 Wiki。"),
        "search_web_images" => t("Search web images.", "搜索网络图片。"),
        "print_image" => t("Display a local image.", "显示本地图片。"),
        "generate_image" => t("Generate an image from text.", "根据文本生成图片。"),
        "search_meme" => t("Search the meme library.", "搜索表情库。"),
        "show_meme" => t("Display a meme.", "显示表情。"),
        "recent_meme" => t("Read the latest sent meme.", "读取最近发送的表情。"),
        "add_meme" => t("Add an image to the meme library.", "向表情库添加图片。"),
        "update_meme" => t("Update meme metadata.", "更新表情元数据。"),
        "delete_meme" => t("Delete a meme entry.", "删除表情条目。"),
        "remember_fact" => t("Save a long-term memory.", "保存长期记忆。"),
        "search_evicted_context" => t(
            "Search removed conversation context.",
            "搜索已移出上下文的会话。",
        ),
        "recall_past_events" => t("Search past conversation events.", "搜索过往会话事件。"),
        "recall_memory" | "recall_memories" => t("Search saved memories.", "搜索已保存记忆。"),
        "forget_memory" | "forget_memories" => t("Forget saved memories.", "遗忘已保存记忆。"),
        "list_memory" | "list_memories" => t("List saved memories.", "列出已保存记忆。"),
        "upload_knowledge_base_file" | "upload_text_to_knowledge_base" => t(
            "Create or replace a knowledge-base file.",
            "创建或替换知识库文件。",
        ),
        "edit_knowledge_base_file" => t("Edit a knowledge-base file.", "编辑知识库文件。"),
        "remove_knowledge_base_file" => t("Remove a knowledge-base file.", "移除知识库文件。"),
        "search_knowledge_base" => t("Search knowledge-base content.", "搜索知识库内容。"),
        "search_knowledge_base_by_name" => t(
            "Find knowledge-base files by name.",
            "按名称查找知识库文件。",
        ),
        "read_knowledge_base_file" => t("Read a knowledge-base file.", "读取知识库文件。"),
        "list_knowledge_base_files" => t("List knowledge-base files.", "列出知识库文件。"),
        "aur_search_packages" => t("Search AUR packages.", "搜索 AUR 软件包。"),
        "aur_get_package_info" => t("Read AUR package details.", "读取 AUR 软件包详情。"),
        "aur_check_status" => t(
            "Read Arch and AUR service status.",
            "查询 Arch 与 AUR 服务状态。",
        ),
        "archlinux_official_package_query" => {
            t("Search official Arch packages.", "搜索 Arch 官方软件包。")
        }
        "archwiki_query" => t("Search or read ArchWiki.", "搜索或读取 ArchWiki。"),
        "online_man_search" | "man_search" => t("Search Linux manual pages.", "搜索 Linux 手册。"),
        "online_man_get_page" | "man_read" => t("Read a Linux manual page.", "读取 Linux 手册页。"),
        "query_moegirl" | "moegirl_query" => {
            t("Search or read Moegirlpedia.", "搜索或读取萌娘百科。")
        }
        "review_aur_package" | "review_pkgbuild_directory" => t(
            "Review AUR package build files.",
            "审查 AUR 软件包构建文件。",
        ),
        "install_aur_package" => t(
            "Install a reviewed AUR package.",
            "安装已审查的 AUR 软件包。",
        ),
        "scientific_calculator" | "calculator" | "calculate" | "calculate_expression" => {
            t("Evaluate numeric expressions.", "计算数值表达式。")
        }
        "calculate_hash" => t("Calculate text or byte hashes.", "计算文本或字节哈希。"),
        "decode_encoded_text" => t("Decode encoded text.", "解码文本。"),
        "protondb_query" => t(
            "Read ProtonDB compatibility reports.",
            "查询 ProtonDB 兼容性报告。",
        ),
        "linux_game_compatibility" => t(
            "Investigate Linux game compatibility.",
            "调查 Linux 游戏兼容性。",
        ),
        "gather_linux_game_compatibility_signals" => t(
            "Collect Linux game compatibility signals.",
            "收集 Linux 游戏兼容性信号。",
        ),
        "register_linux_game_evidence" => t(
            "Record game compatibility evidence.",
            "登记游戏兼容性证据。",
        ),
        "check_issue" => t("Collect local diagnostic evidence.", "收集本地诊断证据。"),
        "linux_input_method_diagnose" | "deep_diagnose" => t(
            "Diagnose Linux input method issues.",
            "诊断 Linux 输入法问题。",
        ),
        "set_alarm" => t("Set a local alarm.", "设置本地闹钟。"),
        "list_alarms" => t("List local alarms.", "列出本地闹钟。"),
        "cancel_alarm" => t("Cancel a local alarm.", "取消本地闹钟。"),
        "draw_zhouyi_hexagram" | "xuanxue_pick" => {
            t("Draw a Zhouyi hexagram.", "随机抽取周易卦象。")
        }
        "draw_tarot_card" => t("Draw a tarot card.", "随机抽取塔罗牌。"),
        "draw_fortune_lot" | "xuanxue_divine" => t("Draw a fortune result.", "随机抽取吉凶结果。"),
        "roll_dice" => t("Roll dice.", "掷骰子。"),
        "send_channel_message" => t("Send a channel message.", "发送渠道消息。"),
        "send_channel_image" => t("Send a channel image.", "发送渠道图片。"),
        "send_channel_file" => t("Send a channel file.", "发送渠道文件。"),
        "send_channel_video" => t("Send a channel video.", "发送渠道视频。"),
        _ => return fallback.to_string(),
    };
    description.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_tools_use_explicit_short_descriptions() {
        let description = tool_description("run_command", "long fallback");
        assert_ne!(description, "long fallback");
        assert!(description.chars().count() < 48);
    }

    #[test]
    fn dynamic_tools_keep_their_server_description() {
        let description = tool_description("mcp_example_lookup", "Server-provided details.");
        assert_eq!(description, "Server-provided details.");
    }

    #[test]
    fn progressive_loader_keeps_its_dynamic_catalog() {
        let description = tool_description("load", "Available groups: web_search");
        assert_eq!(description, "Available groups: web_search");
    }
}
