use crate::i18n::{locale, Locale};

/// 返回面向用户展示的工具名称。
///
/// 参数:
/// - `name`: 工具原始名称
///
/// 返回:
/// - 本地化后的工具名称，未知工具返回原名
pub(crate) fn readable_tool_name(name: &str) -> &str {
    readable_tool_name_for_locale(name, locale())
}

/// 按指定语言返回面向用户展示的工具名称。
///
/// 参数:
/// - `name`: 工具原始名称
/// - `locale`: 展示语言
///
/// 返回:
/// - 本地化后的工具名称，未知工具返回原名
fn readable_tool_name_for_locale(name: &str, locale: Locale) -> &str {
    match name {
        "run_command" => localized(locale, "Run command", "运行命令"),
        "background_command" => localized(locale, "Background command", "后台命令"),
        "subagent" => localized(locale, "Subagent", "子智能体"),
        "todo" => localized(locale, "Todo list", "待办清单"),
        "cron" => localized(locale, "Scheduled task", "定时任务"),
        "read_file" => localized(locale, "Read file", "读取文件"),
        "edit_file" => localized(locale, "Edit file", "编辑文件"),
        "list_directory" => localized(locale, "List directory", "列目录"),
        "create_directory" => localized(locale, "Create directory", "创建目录"),
        "trash_path" => localized(locale, "Move to trash", "移入回收站"),
        "find_files" | "glob" => localized(locale, "Find files", "查找文件"),
        "search_text" | "grep" => localized(locale, "Search text", "搜索文本"),
        "get_current_directory" => localized(locale, "Current directory", "当前目录"),
        "get_current_time" => localized(locale, "Current time", "当前时间"),
        "inspect_issue" => localized(locale, "Inspect issue", "检查问题"),
        "check_os_info" => localized(locale, "Check system info", "查看系统信息"),
        "web_search" => localized(locale, "Web search", "网页搜索"),
        "web_fetch" => localized(locale, "Read webpage", "读取网页"),
        "search_web_images" => localized(locale, "Search images", "搜索图片"),
        "print_image" => localized(locale, "Show image", "显示图片"),
        "generate_image" => localized(locale, "Generate image", "生成图片"),
        "search_meme" => localized(locale, "Search memes", "搜索表情包"),
        "show_meme" => localized(locale, "Send meme", "发送表情"),
        "add_meme" => localized(locale, "Add meme", "添加表情包"),
        "update_meme" => localized(locale, "Update meme", "更新表情包"),
        "delete_meme" => localized(locale, "Delete meme", "删除表情包"),
        "deep_research" => localized(locale, "Deep research", "深度研究"),
        "upload_knowledge_base_file" | "upload_text_to_knowledge_base" => {
            localized(locale, "Import knowledge base", "导入知识库")
        }
        "read_knowledge_base_file" => localized(locale, "Read knowledge base", "读取知识库"),
        "search_knowledge_base" => localized(locale, "Search knowledge base", "搜索知识库"),
        "search_knowledge_base_by_name" => {
            localized(locale, "Search knowledge base by name", "按名称搜索知识库")
        }
        "edit_knowledge_base_file" => localized(locale, "Edit knowledge base", "编辑知识库"),
        "remove_knowledge_base_file" => localized(locale, "Remove knowledge base", "移除知识库"),
        "list_knowledge_base_files" => localized(locale, "List knowledge base", "列出知识库"),
        "set_alarm" => localized(locale, "Set alarm", "设置闹钟"),
        "list_alarms" => localized(locale, "List alarms", "列出闹钟"),
        "cancel_alarm" => localized(locale, "Cancel alarm", "取消闹钟"),
        "remember_fact" => localized(locale, "Remember fact", "记录记忆"),
        "search_evicted_context" => localized(locale, "Search old context", "搜索旧上下文"),
        "recall_past_events" => localized(locale, "Recall past events", "回忆往事"),
        "recall_memory" | "recall_memories" => localized(locale, "Recall memory", "召回记忆"),
        "forget_memory" | "forget_memories" => localized(locale, "Forget memory", "删除记忆"),
        "list_memory" | "list_memories" => localized(locale, "List memory", "列出记忆"),
        "aur_search_packages" => localized(locale, "Search AUR", "搜索 AUR"),
        "aur_get_package_info" => localized(locale, "View AUR package", "查看 AUR 包"),
        "aur_check_status" => localized(locale, "Check AUR status", "查询 AUR 状态"),
        "pacman_search" => localized(locale, "Search packages", "搜索软件包"),
        "archwiki_query" => localized(locale, "Query ArchWiki", "查询 ArchWiki"),
        "online_man_search" | "man_search" => {
            localized(locale, "Search online manuals", "搜索在线手册")
        }
        "online_man_get_page" | "man_read" => {
            localized(locale, "Read online manual", "读取在线手册")
        }
        "moegirl_query" => localized(locale, "Query Moegirl", "查询萌娘百科"),
        "calculate" | "calculator" => localized(locale, "Calculate", "计算"),
        "calculate_hash" => localized(locale, "Calculate hash", "计算哈希"),
        "decode_encoded_text" => localized(locale, "Decode text", "解码文本"),
        "exchange_rate" | "get_exchange_rate" => localized(locale, "Exchange rate", "汇率查询"),
        "weather" | "get_weather" => localized(locale, "Weather", "天气查询"),
        "xuanxue_pick" => localized(locale, "Mystic pick", "玄学选择"),
        "xuanxue_divine" => localized(locale, "Mystic divination", "玄学占卜"),
        "draw_zhouyi_hexagram" => localized(locale, "Draw Zhouyi hexagram", "周易起卦"),
        "draw_tarot_card" => localized(locale, "Draw tarot card", "抽塔罗牌"),
        "draw_fortune_lot" => localized(locale, "Draw fortune lot", "抽签"),
        "load" => localized(locale, "Load", "加载"),
        "review_aur_package" => localized(locale, "Review AUR package", "审查 AUR 包"),
        "install_aur_package" => localized(locale, "Install AUR package", "安装 AUR 包"),
        "review_pkgbuild_directory" => {
            localized(locale, "Review PKGBUILD directory", "审查 PKGBUILD 目录")
        }
        "linux_game_compatibility" => localized(
            locale,
            "Check Linux game compatibility",
            "查询 Linux 游戏兼容性",
        ),
        _ => name,
    }
}

/// 按语言选择静态展示文本。
///
/// 参数:
/// - `locale`: 展示语言
/// - `en`: 英文文本
/// - `zh`: 中文文本
///
/// 返回:
/// - 与语言匹配的文本
fn localized(locale: Locale, en: &'static str, zh: &'static str) -> &'static str {
    match locale {
        Locale::En => en,
        Locale::Zh => zh,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readable_tool_names_translate_known_tools_to_chinese() {
        assert_eq!(
            readable_tool_name_for_locale("deep_research", Locale::Zh),
            "深度研究"
        );
        assert_eq!(
            readable_tool_name_for_locale("read_file", Locale::Zh),
            "读取文件"
        );
        assert_eq!(
            readable_tool_name_for_locale("inspect_issue", Locale::Zh),
            "检查问题"
        );
        assert_eq!(
            readable_tool_name_for_locale("check_os_info", Locale::Zh),
            "查看系统信息"
        );
        assert_eq!(
            readable_tool_name_for_locale("get_weather", Locale::Zh),
            "天气查询"
        );
        assert_eq!(
            readable_tool_name_for_locale("get_exchange_rate", Locale::Zh),
            "汇率查询"
        );
        assert_eq!(
            readable_tool_name_for_locale("draw_zhouyi_hexagram", Locale::Zh),
            "周易起卦"
        );
        assert_eq!(
            readable_tool_name_for_locale("draw_tarot_card", Locale::Zh),
            "抽塔罗牌"
        );
        assert_eq!(
            readable_tool_name_for_locale("draw_fortune_lot", Locale::Zh),
            "抽签"
        );
        assert_eq!(
            readable_tool_name_for_locale("search_meme", Locale::Zh),
            "搜索表情包"
        );
        assert_eq!(
            readable_tool_name_for_locale("show_meme", Locale::Zh),
            "发送表情"
        );
        assert_eq!(
            readable_tool_name_for_locale("add_meme", Locale::Zh),
            "添加表情包"
        );
        assert_eq!(
            readable_tool_name_for_locale("subagent", Locale::Zh),
            "子智能体"
        );
        assert_eq!(
            readable_tool_name_for_locale("upload_text_to_knowledge_base", Locale::Zh),
            "导入知识库"
        );
        assert_eq!(
            readable_tool_name_for_locale("search_evicted_context", Locale::Zh),
            "搜索旧上下文"
        );
        assert_eq!(
            readable_tool_name_for_locale("recall_past_events", Locale::Zh),
            "回忆往事"
        );
        assert_eq!(
            readable_tool_name_for_locale("aur_check_status", Locale::Zh),
            "查询 AUR 状态"
        );
        assert_eq!(
            readable_tool_name_for_locale("online_man_search", Locale::Zh),
            "搜索在线手册"
        );
        assert_eq!(
            readable_tool_name_for_locale("online_man_get_page", Locale::Zh),
            "读取在线手册"
        );
        assert_eq!(
            readable_tool_name_for_locale("install_aur_package", Locale::Zh),
            "安装 AUR 包"
        );
        assert_eq!(
            readable_tool_name_for_locale("search_knowledge_base_by_name", Locale::Zh),
            "按名称搜索知识库"
        );
        assert_eq!(
            readable_tool_name_for_locale("recall_memories", Locale::Zh),
            "召回记忆"
        );
        assert_eq!(
            readable_tool_name_for_locale("custom_skill", Locale::Zh),
            "custom_skill"
        );
    }

    #[test]
    fn readable_tool_names_translate_known_tools_to_english() {
        assert_eq!(
            readable_tool_name_for_locale("deep_research", Locale::En),
            "Deep research"
        );
        assert_eq!(
            readable_tool_name_for_locale("read_file", Locale::En),
            "Read file"
        );
        assert_eq!(
            readable_tool_name_for_locale("get_weather", Locale::En),
            "Weather"
        );
        assert_eq!(
            readable_tool_name_for_locale("run_command", Locale::En),
            "Run command"
        );
        assert_eq!(
            readable_tool_name_for_locale("background_command", Locale::En),
            "Background command"
        );
        assert_eq!(
            readable_tool_name_for_locale("custom_skill", Locale::En),
            "custom_skill"
        );
    }
}
