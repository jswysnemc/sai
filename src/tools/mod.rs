mod alarm;
mod archlinux;
mod ask_question;
mod calculator;
mod catalog;
pub(crate) mod command;
mod context;
mod deep_diagnose;
mod deep_research;
mod deepseek_status;
mod default_tools;
mod diagnostics;
mod edit_file;
pub(crate) mod edit_patch;
mod exchange_rate;
mod fcitx_wiki;
mod file_read;
pub(crate) mod groups;
mod hash_codec;
mod image_generation;
pub mod knowledge_base;
mod linux_game;
mod man;
pub(crate) mod memes;
mod memory;
mod moegirl;
mod native_search;
mod package_advisor;
pub(crate) mod progressive;
mod protondb_query;
mod registry;
mod skill_management;
mod skills;
mod subagent;
pub(crate) mod subagent_event;
mod subagent_feed;
pub(crate) mod subagent_goal;
mod subagent_persistence;
mod subagent_runner;
mod subagent_runtime;
pub(crate) mod subagent_state;
pub(crate) mod subagent_timeline;
mod subagent_worktree;
pub(crate) mod todo;
mod trash_path;
mod vision;
mod weather;
mod web;
mod web_images;
mod xuanxue;

use crate::config::AppConfig;
use crate::paths::SaiPaths;
pub(crate) use catalog::{mcp_tool_catalog, tool_catalog, ToolCatalogEntry};
pub(crate) use context::tool_output_for_context;
pub(crate) use progressive::{is_initial_tool, register_loader as register_progressive_loader, LOAD_NAME};
pub use registry::{empty_parameters, ToolPermission, ToolProgress, ToolRegistry, ToolSpec};
pub(crate) use registry::{ToolModelAttachment, ToolOutput};
pub(crate) use skill_management::{
    create_managed_skill, list_managed_skills, read_managed_skill, set_managed_skill_enabled,
    update_managed_skill, ManagedSkill,
};
pub(crate) use skills::load_installed_skill;
pub use skills::{
    load_installed_skill_document, register_skills, skill_catalog, skills_catalog_prompt,
    skills_prompt,
};

pub fn readable_tool_name(name: &str) -> &str {
    match name {
        "run_command" => "运行命令",
        "background_command" => "后台命令",
        "subagent" => "子智能体",
        "todo" => "任务清单",
        "cron" => "定时任务",
        "read_file" => "读取文件",
        "edit_file" => "编辑文件",
        "create_goal" => "创建目标",
        "get_goal" => "查看目标",
        "update_goal" => "更新目标",
        "list_directory" => "列目录",
        "create_directory" => "创建目录",
        "trash_path" => "移入回收站",
        "find_files" | "glob" => "查找文件",
        "search_text" | "grep" => "搜索文本",
        "get_current_directory" => "当前目录",
        "get_current_time" => "当前时间",
        "check_issue" => "检查问题",
        "check_os_info" => "查看系统信息",
        "web_search" => "网页搜索",
        "web_fetch" => "读取网页",
        "fcitx5_input_method_wiki_qurey" => "查询 Fcitx5 Wiki",
        "search_web_images" => "搜索图片",
        "print_image" => "显示图片",
        "generate_image" => "生成图片",
        "search_meme" => "搜索表情包",
        "show_meme" => "发送表情",
        "add_meme" => "添加表情包",
        "update_meme" => "更新表情包",
        "delete_meme" => "删除表情包",
        "deep_research" => "深度研究",
        "deep_diagnose" | "linux_input_method_diagnose" => "输入法诊断",
        "upload_knowledge_base_file" | "upload_text_to_knowledge_base" => "导入知识库",
        "read_knowledge_base_file" => "读取知识库",
        "search_knowledge_base" => "搜索知识库",
        "search_knowledge_base_by_name" => "按名称搜索知识库",
        "edit_knowledge_base_file" => "编辑知识库",
        "remove_knowledge_base_file" => "移除知识库",
        "list_knowledge_base_files" => "列出知识库",
        "set_alarm" => "设置闹钟",
        "list_alarms" => "列出闹钟",
        "cancel_alarm" => "取消闹钟",
        "remember_fact" => "记录记忆",
        "search_evicted_context" => "搜索旧上下文",
        "recall_past_events" => "回忆往事",
        "recall_memory" | "recall_memories" => "召回记忆",
        "forget_memory" | "forget_memories" => "删除记忆",
        "list_memory" | "list_memories" => "列出记忆",
        "aur_search_packages" => "搜索 AUR",
        "aur_get_package_info" => "查看 AUR 包",
        "aur_check_status" => "查询 AUR 状态",
        "archlinux_official_package_query" => "查询 Arch 官方包",
        "query_deepseek_status" => "查询 DeepSeek 状态",
        "pacman_search" => "搜索软件包",
        "archwiki_query" => "查询 ArchWiki",
        "online_man_search" | "man_search" => "搜索在线手册",
        "online_man_get_page" | "man_read" => "读取在线手册",
        "moegirl_query" => "查询萌娘百科",
        "calculate" | "calculator" => "计算",
        "calculate_hash" => "计算哈希",
        "decode_encoded_text" => "解码文本",
        "exchange_rate" | "get_exchange_rate" => "汇率查询",
        "weather" | "get_weather" => "天气查询",
        "protondb_query" => "查询 ProtonDB",
        "xuanxue_pick" => "玄学选择",
        "xuanxue_divine" => "玄学占卜",
        "draw_zhouyi_hexagram" => "周易起卦",
        "draw_tarot_card" => "抽塔罗牌",
        "draw_fortune_lot" => "吉凶占",
        "roll_dice" => "掷骰子",
        "load" => "加载",
        "review_aur_package" => "审查 AUR 包",
        "install_aur_package" => "安装 AUR 包",
        "review_pkgbuild_directory" => "审查 PKGBUILD 目录",
        "linux_game_compatibility" => "查询 Linux 游戏兼容性",
        "gather_linux_game_compatibility_signals" => "收集游戏兼容性",
        "register_linux_game_evidence" => "登记兼容性证据",
        "register_deep_research_topic_title" => "注册研究标题",
        "register_deep_research_reference" => "注册引用来源",
        "remove_deep_research_reference" => "移除引用来源",
        "send_channel_image" => "发送渠道图片",
        "send_channel_file" => "发送渠道文件",
        "send_channel_video" => "发送渠道视频",
        _ => name,
    }
}

pub fn clear_aur_review_state(paths: &SaiPaths) -> anyhow::Result<()> {
    package_advisor::clear_aur_review_state(paths)
}

/// 构建完整工具注册表，包括外部 MCP 动态工具。
///
/// 参数:
/// - `config`: 当前应用配置
/// - `paths`: 应用目录路径集合
///
/// 返回:
/// - 可直接用于 Agent 运行的完整工具注册表
pub fn builtin_registry(config: &AppConfig, paths: &SaiPaths) -> ToolRegistry {
    let mut registry = builtin_registry_without_mcp(config, paths);
    crate::mcp::register_mcp_tools(&mut registry, config, paths);
    registry
}

/// 构建使用缓存定义并延迟连接 MCP 的工具注册表。
///
/// 参数:
/// - `config`: 当前应用配置
/// - `paths`: 应用目录路径集合
///
/// 返回:
/// - 内置工具与缓存 MCP 工具组成的注册表
pub(crate) fn builtin_registry_with_cached_mcp(
    config: &AppConfig,
    paths: &SaiPaths,
) -> ToolRegistry {
    let mut registry = builtin_registry_without_mcp(config, paths);
    crate::mcp::register_cached_mcp_tools(&mut registry, config, paths);
    registry
}

/// 构建不触发 MCP 发现的本地工具注册表。
///
/// 参数:
/// - `config`: 当前应用配置
/// - `paths`: 应用目录路径集合
///
/// 返回:
/// - 内置工具与 MCP 管理工具组成的注册表
pub(crate) fn builtin_registry_without_mcp(config: &AppConfig, paths: &SaiPaths) -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    command::register(&mut registry, config, paths, true);
    default_tools::register(&mut registry, config, paths);
    trash_path::register(&mut registry);
    alarm::register(&mut registry, paths.clone());
    web::register_fetch(&mut registry);
    fcitx_wiki::register(&mut registry);
    weather::register(&mut registry);
    protondb_query::register(&mut registry);
    exchange_rate::register(&mut registry, config.plugins.exchange_rate.clone());
    xuanxue::register(&mut registry);
    if config.plugins.archlinux.enabled {
        archlinux::register(&mut registry);
    }
    if config.plugins.man.enabled {
        man::register(&mut registry);
    }
    moegirl::register(&mut registry);
    hash_codec::register(&mut registry);
    calculator::register(&mut registry);
    deepseek_status::register(&mut registry);
    vision::register_print(&mut registry, config.clone());
    if config.plugins.memes.enabled {
        memes::register(&mut registry, config.clone(), paths.clone());
    }
    if config.plugins.web.enabled {
        web::register(&mut registry, config.plugins.web.clone());
    }
    if config.plugins.web_images.enabled {
        web_images::register(&mut registry, config.clone(), paths.clone(), true);
    }
    if config.plugins.deep_research.enabled {
        let research_tools = registry.clone();
        deep_research::register(&mut registry, config.clone(), paths.clone(), research_tools);
    }
    if config.plugins.deep_diagnose.enabled {
        let diagnosis_tools = registry.clone();
        deep_diagnose::register(
            &mut registry,
            config.clone(),
            paths.clone(),
            diagnosis_tools,
        );
    }
    if config.plugins.image_generation.enabled {
        image_generation::register(&mut registry, config.clone());
    }
    if config.plugins.knowledge_base.enabled {
        knowledge_base::register(&mut registry, config.clone(), paths.clone());
    }
    if config.plugins.package_advisor.enabled {
        package_advisor::register(&mut registry, paths.clone());
    }
    if config.plugins.linux_game_compatibility.enabled {
        let game_tools = registry.clone();
        linux_game::register(&mut registry, config.clone(), paths.clone(), game_tools);
    }
    if config.plugins.diagnostics.enabled {
        diagnostics::register(&mut registry, config.clone());
    }
    if config.memory_config().enabled {
        memory::register(&mut registry, config.clone(), paths.clone());
    }
    crate::mcp::register_mcp_manager(&mut registry, paths.clone());
    registry
}

/// 将后台命令工具绑定到命令模式运行时 owner。
///
/// 参数:
/// - `registry`: 工具注册表
/// - `config`: 应用配置
/// - `paths`: Sai 路径
/// - `session_id`: 会话标识
pub(crate) fn register_command_mode_background(
    registry: &mut ToolRegistry,
    config: &AppConfig,
    paths: &SaiPaths,
    session_id: &str,
) {
    command::register_command_mode_background(registry, config, paths, session_id);
}

/// 注册仅限交互式会话使用的工具。
///
/// 参数:
/// - `registry`: 当前交互式会话工具注册表
/// - `config`: 应用配置
/// - `paths`: Sai 路径
///
/// 返回:
/// - 无
pub(crate) fn register_interactive_tools(
    registry: &mut ToolRegistry,
    config: &AppConfig,
    paths: &SaiPaths,
    owner_key: String,
    session_id: String,
) {
    command::register_session_background(registry, config, paths, &session_id);
    let subagent_tools = registry.clone();
    subagent::register(
        registry,
        config.clone(),
        paths.clone(),
        subagent_tools,
        owner_key.clone(),
        session_id,
    );
    todo::register(
        registry,
        std::path::PathBuf::from(owner_key).join("todos.json"),
    );
    register_ask_question(registry);
}

/// 注册结构化提问工具。
///
/// 参数:
/// - `registry`: 工具注册表
///
/// 返回:
/// - 无
pub(crate) fn register_ask_question(registry: &mut ToolRegistry) {
    if !registry.contains("ask_question") {
        ask_question::register(registry);
    }
}

pub fn readonly_registry(config: &AppConfig, paths: &SaiPaths) -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    command::register_readonly(&mut registry, config, paths);
    default_tools::register_readonly(&mut registry, config, paths);
    web::register_fetch(&mut registry);
    fcitx_wiki::register(&mut registry);
    protondb_query::register(&mut registry);
    if config.plugins.archlinux.enabled {
        archlinux::register(&mut registry);
    }
    if config.plugins.man.enabled {
        man::register(&mut registry);
    }
    if config.plugins.web.enabled {
        web::register(&mut registry, config.plugins.web.clone());
    }
    if config.plugins.web_images.enabled {
        web_images::register(&mut registry, config.clone(), paths.clone(), false);
    }
    if config.plugins.knowledge_base.enabled {
        knowledge_base::register_readonly(&mut registry, config.clone(), paths.clone());
    }
    if config.plugins.package_advisor.enabled {
        package_advisor::register(&mut registry, paths.clone());
    }
    if config.plugins.linux_game_compatibility.enabled {
        let game_tools = registry.clone();
        linux_game::register(&mut registry, config.clone(), paths.clone(), game_tools);
    }
    if config.plugins.diagnostics.enabled {
        diagnostics::register(&mut registry, config.clone());
    }
    if config.memory_config().enabled {
        memory::register_readonly(&mut registry, config.clone(), paths.clone());
    }
    registry
}
