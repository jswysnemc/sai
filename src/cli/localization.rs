use super::args::Cli;
use crate::i18n::{apply_locale_override_from_args, is_zh, text as t};
use clap::{Arg, ArgAction, CommandFactory, FromArgMatches};

mod background;
mod gateway;

use self::background::localize_background_commands_command;
use self::gateway::localize_gateway_command;

pub(crate) fn parse() -> Cli {
    apply_locale_override_from_args(std::env::args_os());
    let matches = localized_command().get_matches();
    Cli::from_arg_matches(&matches).unwrap_or_else(|err| err.exit())
}

fn localized_command() -> clap::Command {
    let mut command = Cli::command();
    command = command
        .about(t("Sai CLI AI Agent", "Sai 命令行 AI 助手"))
        .override_usage(t(
            "sai [OPTIONS] [MESSAGE]... [COMMAND]",
            "sai [选项] [消息]... [命令]",
        ));
    if is_zh() {
        command = command
            .subcommand_help_heading("命令")
            .arg_required_else_help(false)
            .next_help_heading("选项")
            .help_template("{about}\n\n用法: {usage}\n\n命令:\n{subcommands}\n参数:\n{positionals}\n选项:\n{options}\n{after-help}")
            .after_help("提示：不带参数进入 REPL；直接输入消息会发送一次对话。使用 SAI_LANG=en_US 可切换英文。")
            .disable_help_subcommand(true);
    } else {
        command = command
            .after_help(
                "Tip: run without arguments to enter the REPL; pass MESSAGE to send one chat turn. Set SAI_LANG=zh_CN for Chinese.",
            )
            .disable_help_subcommand(true);
    }
    command = localize_top_args(command);
    command = localize_subcommands(command);
    command = apply_localized_help_flags(command, true);
    if is_zh() {
        command = apply_chinese_help_template(command);
    }
    command
}

fn apply_localized_help_flags(mut command: clap::Command, root: bool) -> clap::Command {
    command = command.disable_help_flag(true).arg(
        Arg::new("help")
            .short('h')
            .long("help")
            .help(t("Print help", "显示帮助"))
            .action(ArgAction::Help),
    );
    if root {
        command = command.disable_version_flag(true).arg(
            Arg::new("version")
                .short('V')
                .long("version")
                .help(t("Print version", "显示版本"))
                .action(ArgAction::Version),
        );
    }
    let subcommands = command
        .get_subcommands()
        .map(|subcommand| subcommand.get_name().to_string())
        .collect::<Vec<_>>();
    for name in subcommands {
        command = command.mut_subcommand(&name, |subcommand| {
            apply_localized_help_flags(subcommand, false)
        });
    }
    command
}

fn apply_chinese_help_template(mut command: clap::Command) -> clap::Command {
    let has_subcommands = command.get_subcommands().next().is_some();
    command = if has_subcommands {
        command.help_template(
            "{about}\n\n用法: {usage}\n\n命令:\n{subcommands}\n参数:\n{positionals}\n选项:\n{options}\n{after-help}",
        )
    } else {
        command.help_template(
            "{about}\n\n用法: {usage}\n\n参数:\n{positionals}\n选项:\n{options}\n{after-help}",
        )
    };
    let subcommands = command
        .get_subcommands()
        .map(|subcommand| subcommand.get_name().to_string())
        .collect::<Vec<_>>();
    for name in subcommands {
        command = command.mut_subcommand(&name, apply_chinese_help_template);
    }
    command
}

fn localize_top_args(command: clap::Command) -> clap::Command {
    command
        .mut_arg("lang", |arg| {
            arg.help(t(
                "Interface language: en-US or zh-CN",
                "界面语言：en-US 或 zh-CN",
            ))
        })
        .mut_arg("plan", |arg| {
            arg.help(t("Run in read-only planning mode", "使用只读计划模式运行"))
        })
        .mut_arg("audited", |arg| {
            arg.help(t(
                "Run with audit logging and workspace sandboxing",
                "使用审计日志和工作区沙盒运行",
            ))
        })
        .mut_arg("yolo", |arg| {
            arg.help(t(
                "Run without interactive permission prompts",
                "运行时不显示交互式权限询问",
            ))
        })
        .mut_arg("clipb", |arg| {
            arg.help(t(
                "Inject clipboard text or attach clipboard image to the prompt",
                "将剪贴板文本注入提示词，或将剪贴板图片附加到提示词",
            ))
        })
        .mut_arg("web_search", |arg| {
            arg.help(t("Enable web search for this message", "为本次消息启用网页搜索"))
        })
        .mut_arg("thinking", |arg| {
            arg.help(t(
                "Temporarily override model thinking level: auto, none, low, medium, high, xhigh, or max",
                "临时覆盖模型思考等级：auto、none、low、medium、high、xhigh 或 max",
            ))
        })
        .mut_arg("message", |arg| {
            arg.help(t(
                "Message to send; omitted to enter REPL",
                "要发送的消息；省略则进入 REPL",
            ))
        })
}

fn localize_subcommands(mut command: clap::Command) -> clap::Command {
    let descriptions = [
        ("web", "Start the Sai Web coding workspace", "启动 Sai Web 编程工作台"),
        (
            "ask",
            "Send one message to the assistant",
            "向助手发送一条消息",
        ),
        (
            "init",
            "Create default config and state files",
            "创建默认配置和状态文件",
        ),
        (
            "paths",
            "Show app config, data, and cache paths",
            "显示应用配置、数据和缓存路径",
        ),
        ("config", "Open or manage configuration", "打开或管理配置"),
        (
            "providers",
            "List or switch provider/model",
            "列出或切换 provider/模型",
        ),
        (
            "fish-init",
            "Integrate with fish so you can chat in natural language directly in the terminal",
            "集成到 fish，集成后可在终端直接使用自然语言交流。",
        ),
        (
            "bash-init",
            "Integrate with bash so you can chat in natural language directly in the terminal",
            "集成到 bash，集成后可在终端直接使用自然语言交流。",
        ),
        (
            "zsh-init",
            "Integrate with zsh so you can chat in natural language directly in the terminal",
            "集成到 zsh，集成后可在终端直接使用自然语言交流。",
        ),
        (
            "powershell-init",
            "Integrate with PowerShell so you can chat in natural language directly in the terminal",
            "集成到 PowerShell，集成后可在终端直接使用自然语言交流。",
        ),
        (
            "remove-shell-hook",
            "Safely remove installed Sai shell hooks",
            "安全删除已安装的 Sai shell hook",
        ),
        ("history", "Show conversation history", "显示会话历史"),
        ("sessions", "Manage saved sessions", "管理已保存会话"),
        ("resume", "Resume a session by selection or ID", "交互选择或按 ID 恢复会话"),
        ("kb", "Manage local knowledge base", "管理本地知识库"),
        (
            "memory",
            "Inspect or edit assistant memory",
            "查看或编辑助手记忆",
        ),
        ("skills", "Manage assistant skills", "管理助手 skills"),
        ("ps", "Manage background commands", "管理后台命令"),
        (
            "gateway",
            "Send messages through WeCom, QQ official bot, or OneBot gateways",
            "通过企业微信、QQ 官方机器人或 OneBot 网关发送消息",
        ),
        (
            "weixin-login",
            "Log in to Weixin iLink by QR code",
            "通过二维码登录微信 iLink",
        ),
        ("set", "Set active configuration values", "设置当前配置项"),
        ("clear", "Clear current conversation history", "清空当前会话历史"),
        (
            "compact",
            "Manually compact old conversation turns",
            "手动压缩旧会话轮次",
        ),
    ];
    for (name, en, zh) in descriptions {
        command = command.mut_subcommand(name, |subcommand| subcommand.about(t(en, zh)));
    }
    command = command
        .mut_subcommand("web", localize_web_command)
        .mut_subcommand("ask", localize_ask_command)
        .mut_subcommand("providers", localize_providers_command)
        .mut_subcommand("history", localize_history_command)
        .mut_subcommand("sessions", localize_sessions_command)
        .mut_subcommand("resume", localize_resume_command)
        .mut_subcommand("kb", localize_kb_command)
        .mut_subcommand("memory", localize_memory_command)
        .mut_subcommand("skills", localize_skills_command)
        .mut_subcommand("ps", localize_background_commands_command)
        .mut_subcommand("gateway", localize_gateway_command)
        .mut_subcommand("weixin-login", localize_weixin_login_command)
        .mut_subcommand("set", localize_set_command)
        .mut_subcommand("config", localize_config_command)
        .mut_subcommand("clear", localize_clear_command)
        .mut_subcommand("compact", localize_compact_command);
    command
}

/// 本地化 Web 工作台命令参数。
///
/// 参数:
/// - `command`: Clap Web 子命令
///
/// 返回:
/// - 已本地化的子命令
fn localize_web_command(command: clap::Command) -> clap::Command {
    command
        .mut_arg("port", |arg| {
            arg.help(t("HTTP listen port", "HTTP 监听端口"))
        })
        .mut_arg("no_open", |arg| {
            arg.help(t(
                "Do not open the browser automatically",
                "不自动打开浏览器",
            ))
        })
}

fn localize_ask_command(command: clap::Command) -> clap::Command {
    command
        .mut_arg("clipb", |arg| {
            arg.help(t(
                "Inject clipboard text or attach clipboard image to the prompt",
                "将剪贴板文本注入提示词，或将剪贴板图片附加到提示词",
            ))
        })
        .mut_arg("thinking", |arg| {
            arg.help(t(
                "Temporarily override model thinking level: auto, none, low, medium, high, xhigh, or max",
                "临时覆盖模型思考等级：auto、none、low、medium、high、xhigh 或 max",
            ))
        })
        .mut_arg("web_search", |arg| {
            arg.help(t("Enable web search for this message", "为本次消息启用网页搜索"))
        })
        .mut_arg("message", |arg| {
            arg.help(t("Message to send", "要发送的消息"))
        })
}

fn localize_set_command(command: clap::Command) -> clap::Command {
    command.mut_subcommand("thinking", |subcommand| {
        subcommand
            .about(t(
                "Set active provider thinking level",
                "设置当前 provider 的思考等级",
            ))
            .mut_arg("level", |arg| {
                arg.help(t(
                    "Thinking level: auto, none, low, medium, high, xhigh, or max. Omit to select interactively.",
                    "思考等级：auto、none、low、medium、high、xhigh 或 max。不传则交互选择。",
                ))
            })
    })
}

fn localize_providers_command(command: clap::Command) -> clap::Command {
    command.mut_arg("index", |arg| {
        arg.help(t(
            "Provider/model list index to activate",
            "要激活的 provider/模型列表序号",
        ))
    })
}

fn localize_history_command(command: clap::Command) -> clap::Command {
    command
        .mut_arg("limit", |arg| {
            arg.help(t("Number of history entries to show", "显示的历史条数"))
        })
        .mut_arg("raw", |arg| {
            arg.help(t("Print raw JSONL entries", "输出原始 JSONL 条目"))
        })
        .mut_arg("no_thinking", |arg| {
            arg.help(t("Hide stored reasoning", "隐藏已保存的思考内容"))
        })
}

/// 本地化会话管理命令和参数。
///
/// 参数:
/// - `command`: Clap sessions 子命令
///
/// 返回:
/// - 已本地化的子命令
fn localize_sessions_command(mut command: clap::Command) -> clap::Command {
    let descriptions = [
        ("list", "List saved sessions", "列出已保存会话"),
        ("new", "Create a new session", "创建新会话"),
        ("switch", "Switch to a session by ID", "按 ID 切换会话"),
        (
            "resume",
            "Resume a session by selection or ID",
            "交互选择或按 ID 恢复会话",
        ),
        ("current", "Show the current session", "显示当前会话"),
        ("delete", "Delete a session", "删除会话"),
        ("rename", "Rename a session", "重命名会话"),
    ];
    for (name, en, zh) in descriptions {
        command = command.mut_subcommand(name, |subcommand| subcommand.about(t(en, zh)));
    }
    command
        .mut_subcommand("new", |subcommand| {
            subcommand.mut_arg("title", |arg| arg.help(t("Session title", "会话标题")))
        })
        .mut_subcommand("switch", |subcommand| {
            subcommand.mut_arg("id", |arg| arg.help(t("Session ID", "会话 ID")))
        })
        .mut_subcommand("resume", localize_resume_command)
        .mut_subcommand("delete", |subcommand| {
            subcommand.mut_arg("id", |arg| arg.help(t("Session ID", "会话 ID")))
        })
        .mut_subcommand("rename", |subcommand| {
            subcommand
                .mut_arg("id", |arg| arg.help(t("Session ID", "会话 ID")))
                .mut_arg("title", |arg| {
                    arg.help(t("New session title", "新会话标题"))
                })
        })
}

/// 本地化恢复会话命令参数。
///
/// 参数:
/// - `command`: Clap resume 子命令
///
/// 返回:
/// - 已本地化的子命令
fn localize_resume_command(command: clap::Command) -> clap::Command {
    command.mut_arg("id", |arg| {
        arg.help(t(
            "Session ID; omit to choose interactively",
            "会话 ID；省略则进入交互选择",
        ))
    })
}

/// 本地化顶层微信登录命令参数。
///
/// 参数:
/// - `command`: Clap weixin-login 子命令
///
/// 返回:
/// - 已本地化的子命令
fn localize_weixin_login_command(command: clap::Command) -> clap::Command {
    command
        .mut_arg("verbose", |arg| {
            arg.help(t("Show verbose logs", "显示详细日志"))
        })
        .mut_arg("bot_type", |arg| {
            arg.help(t("Weixin bot type", "微信机器人类型"))
        })
        .mut_arg("timeout_secs", |arg| {
            arg.help(t("Login timeout in seconds", "登录超时秒数"))
        })
        .mut_arg("base_url", |arg| {
            arg.help(t("Weixin iLink base URL", "微信 iLink 基础地址"))
        })
}

fn localize_config_command(command: clap::Command) -> clap::Command {
    command
        .mut_subcommand("validate", |subcommand| {
            subcommand.about(t("Validate configuration", "校验配置"))
        })
        .mut_subcommand("paths", |subcommand| {
            subcommand.about(t("Show configuration paths", "显示配置路径"))
        })
}

fn localize_clear_command(command: clap::Command) -> clap::Command {
    command
        .mut_arg("scope", |arg| {
            arg.help(t(
                "all also clears long-term memory",
                "all 同时清空长期记忆",
            ))
        })
        .mut_arg("memory", |arg| {
            arg.help(t(
                "Clear assistant memory without deleting the current session",
                "清空助手记忆，但保留当前会话",
            ))
        })
}

fn localize_compact_command(command: clap::Command) -> clap::Command {
    command
}

fn localize_kb_command(mut command: clap::Command) -> clap::Command {
    let descriptions = [
        ("add", "Add a file or directory", "添加文件或目录"),
        ("list", "List indexed files", "列出已索引文件"),
        ("search", "Search knowledge base content", "搜索知识库内容"),
        ("find", "Find files by name", "按文件名查找文件"),
        ("read", "Read a knowledge base file", "读取知识库文件"),
        ("remove", "Remove a knowledge base file", "移除知识库文件"),
        (
            "reindex",
            "Rebuild keyword index on demand",
            "按需重建关键词索引",
        ),
        ("stats", "Show knowledge base statistics", "显示知识库统计"),
        ("embed", "Manage semantic embeddings", "管理语义嵌入"),
    ];
    for (name, en, zh) in descriptions {
        command = command.mut_subcommand(name, |subcommand| subcommand.about(t(en, zh)));
    }
    command
        .mut_subcommand("add", |subcommand| {
            subcommand
                .mut_arg("path", |arg| arg.help(t("Path to add", "要添加的路径")))
                .mut_arg("recursive", |arg| {
                    arg.help(t(
                        "Compatibility flag; directories are recursive by default",
                        "兼容参数；目录默认递归导入",
                    ))
                })
        })
        .mut_subcommand("search", |subcommand| {
            subcommand
                .mut_arg("query", |arg| arg.help(t("Search query", "搜索查询")))
                .mut_arg("limit", |arg| arg.help(t("Maximum results", "最大结果数")))
        })
        .mut_subcommand("find", |subcommand| {
            subcommand
                .mut_arg("query", |arg| arg.help(t("Filename query", "文件名查询")))
                .mut_arg("limit", |arg| arg.help(t("Maximum results", "最大结果数")))
        })
        .mut_subcommand("read", |subcommand| {
            subcommand
                .mut_arg("file", |arg| {
                    arg.help(t("Knowledge base file name", "知识库文件名"))
                })
                .mut_arg("start", |arg| arg.help(t("Starting line", "起始行")))
                .mut_arg("lines", |arg| arg.help(t("Number of lines", "读取行数")))
        })
        .mut_subcommand("remove", |subcommand| {
            subcommand.mut_arg("file", |arg| arg.help(t("File to remove", "要移除的文件")))
        })
        .mut_subcommand("embed", |subcommand| {
            subcommand.mut_subcommand("reindex", |nested| {
                nested
                    .about(t("Rebuild semantic embeddings", "重建语义嵌入索引"))
                    .mut_arg("quiet", |arg| {
                        arg.help(t("Suppress progress output", "不输出处理进度"))
                    })
            })
        })
}

fn localize_memory_command(mut command: clap::Command) -> clap::Command {
    let descriptions = [
        ("stats", "Show memory statistics", "显示记忆统计"),
        ("reset", "Clear assistant memory", "清空助手记忆"),
        ("search", "Search memories", "搜索记忆"),
        ("remember", "Save a manual fact", "手动保存事实"),
    ];
    for (name, en, zh) in descriptions {
        command = command.mut_subcommand(name, |subcommand| subcommand.about(t(en, zh)));
    }
    command
        .mut_subcommand("reset", |subcommand| {
            subcommand.mut_arg("include_skills", |arg| {
                arg.help(t(
                    "Also remove generated skills",
                    "同时移除自动生成的 skills",
                ))
            })
        })
        .mut_subcommand("search", |subcommand| {
            subcommand
                .mut_arg("query", |arg| arg.help(t("Search query", "搜索查询")))
                .mut_arg("limit", |arg| arg.help(t("Maximum results", "最大结果数")))
                .mut_arg("forgotten", |arg| {
                    arg.help(t("Include forgotten memories", "包含已遗忘记忆"))
                })
        })
        .mut_subcommand("remember", |subcommand| {
            subcommand
                .mut_arg("content", |arg| arg.help(t("Fact content", "事实内容")))
                .mut_arg("source", |arg| arg.help(t("Source label", "来源标签")))
        })
}

fn localize_skills_command(mut command: clap::Command) -> clap::Command {
    let descriptions = [
        ("list", "List skills", "列出 skills"),
        ("show", "Show a skill", "显示 skill"),
        ("enable", "Enable a skill", "启用 skill"),
        ("disable", "Disable a skill", "禁用 skill"),
        ("remove", "Remove a skill", "移除 skill"),
        ("stats", "Show skill statistics", "显示 skill 统计"),
        (
            "prune",
            "Remove disabled generated skills",
            "清理已禁用的自动 skills",
        ),
    ];
    for (name, en, zh) in descriptions {
        command = command.mut_subcommand(name, |subcommand| subcommand.about(t(en, zh)));
    }
    for name in ["show", "enable", "disable", "remove"] {
        command = command.mut_subcommand(name, |subcommand| {
            subcommand.mut_arg("name", |arg| arg.help(t("Skill name", "skill 名称")))
        });
    }
    command
}
