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
        "run_command" => return run_command_description(),
        "background_command" => return background_command_description(),
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
        "session_probe" => t(
            "Inspect live sessions and their holders.",
            "查看活动会话及其持有者。",
        ),
        "agent_probe" => t(
            "Inspect subagents without disturbing them.",
            "查看子智能体而不打扰它们。",
        ),
        "mesh_send" => t(
            "Send a mesh message and return immediately.",
            "发送网格消息并立即返回。",
        ),
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
        "write_memory" => t(
            "Save one durable fact to memory.",
            "把一条长期事实写入记忆。",
        ),
        "read_memory" => t("Read one memory by identifier.", "按标识读取一条记忆。"),
        "list_memory" => t("List stored memories.", "列出已存记忆。"),
        "delete_memory" => t("Delete a memory.", "删除一条记忆。"),
        "search_evicted_context" => t(
            "Search removed conversation context.",
            "搜索已移出上下文的会话。",
        ),
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

fn run_command_description() -> String {
    #[cfg(windows)]
    {
        return t(
            "Run shell commands. This host is Windows: the command field is already executed as non-interactive PowerShell (pwsh first, then Windows PowerShell, with cmd only as a last fallback). Write the PowerShell script directly; do not prefix it with pwsh/powershell, -Command, -File, cmd /c, or shell launch flags, and do not quote the whole script as a nested command. Do not create a temporary .ps1 file or send it to Trash for a one-off query; run the script directly. Quote Windows paths. Prefer Get-ChildItem, Get-Content, Select-String, Get-Location, Get-Command, and $env:NAME. For process memory statistics use WorkingSet64, PrivateMemorySize64, and VirtualMemorySize64. Do not use POSIX-only flags or syntax such as ls -la, grep -n, sed, awk, export, source, $(...), or bash heredocs; use rg for fast file and text search when available.",
            "执行 Shell 命令。当前主机是 Windows：command 字段本身已经由非交互 PowerShell 执行（优先 pwsh，其次 Windows PowerShell，最后才回退 cmd）。请直接填写 PowerShell 脚本；不要再加 pwsh/powershell、-Command、-File、cmd /c 或其他启动参数，也不要把整段脚本套成嵌套命令。一次性查询不要创建临时 .ps1 文件，也不要把它送进 Trash，直接执行脚本即可。请正确引用 Windows 路径；优先使用 Get-ChildItem、Get-Content、Select-String、Get-Location、Get-Command 和 $env:NAME。统计进程内存必须使用 WorkingSet64、PrivateMemorySize64、VirtualMemorySize64。不要使用 ls -la、grep -n、sed、awk、export、source、$(...)、bash heredoc 等 POSIX 专用参数或语法；文件与文本搜索优先使用 rg。",
        )
        .to_string();
    }
    #[cfg(not(windows))]
    {
        t(
            "Run shell commands using the configured POSIX shell.",
            "使用已配置的 POSIX Shell 执行命令。",
        )
        .to_string()
    }
}

fn background_command_description() -> String {
    #[cfg(windows)]
    {
        return t(
            "Manage background PowerShell commands on Windows.",
            "在 Windows 上管理后台 PowerShell 命令。",
        )
        .to_string();
    }
    #[cfg(not(windows))]
    {
        t("Manage background shell commands.", "管理后台 Shell 命令。").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_tools_use_explicit_short_descriptions() {
        let description = tool_description("run_command", "long fallback");
        assert_ne!(description, "long fallback");
        #[cfg(windows)]
        assert!(description.contains("PowerShell"));
        #[cfg(not(windows))]
        assert!(description.contains("POSIX"));
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
