use super::args::Cli;
use crate::i18n::{is_zh, text as t};
use clap::{Arg, ArgAction, CommandFactory, FromArgMatches};

pub(crate) fn parse() -> Cli {
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
        .mut_arg("plan", |arg| {
            arg.help(t("Run in read-only planning mode", "使用只读计划模式运行"))
        })
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
        .mut_arg("message", |arg| {
            arg.help(t(
                "Message to send; omitted to enter REPL",
                "要发送的消息；省略则进入 REPL",
            ))
        })
}

fn localize_subcommands(mut command: clap::Command) -> clap::Command {
    let descriptions = [
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
        .mut_subcommand("ask", localize_ask_command)
        .mut_subcommand("providers", localize_providers_command)
        .mut_subcommand("history", localize_history_command)
        .mut_subcommand("kb", localize_kb_command)
        .mut_subcommand("memory", localize_memory_command)
        .mut_subcommand("skills", localize_skills_command)
        .mut_subcommand("ps", localize_background_commands_command)
        .mut_subcommand("gateway", localize_gateway_command)
        .mut_subcommand("set", localize_set_command)
        .mut_subcommand("config", localize_config_command)
        .mut_subcommand("clear", localize_clear_command)
        .mut_subcommand("compact", localize_compact_command);
    command
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
        .mut_arg("message", |arg| {
            arg.help(t("Message to send", "要发送的消息"))
        })
}

fn localize_background_commands_command(command: clap::Command) -> clap::Command {
    command
        .mut_subcommand("start", |subcommand| {
            subcommand.about(t(
                "Start a managed background command",
                "启动受管理后台命令",
            ))
        })
        .mut_subcommand("list", |subcommand| {
            subcommand.about(t("List background commands", "列出后台命令"))
        })
        .mut_subcommand("output", |subcommand| {
            subcommand.about(t("Read background command output", "读取后台命令输出"))
        })
        .mut_subcommand("stop", |subcommand| {
            subcommand.about(t("Stop a background command", "停止后台命令"))
        })
        .mut_subcommand("cleanup", |subcommand| {
            subcommand.about(t(
                "Cleanup finished background commands",
                "清理已结束后台命令",
            ))
        })
}

fn localize_gateway_command(command: clap::Command) -> clap::Command {
    command
        .mut_subcommand("start", |subcommand| {
            subcommand.about(t(
                "Start all enabled gateway channels from configuration",
                "启动配置中已启用的所有渠道网关",
            ))
        })
        .mut_subcommand("wecom-webhook", |subcommand| {
            subcommand
                .about(t(
                    "Send text, images, or files through a WeCom group webhook",
                    "通过企业微信群机器人 Webhook 发送文本、图片或文件",
                ))
                .mut_arg("webhook_url", |arg| {
                    arg.help(t("Full WeCom webhook URL", "企业微信 Webhook 完整地址"))
                })
        })
        .mut_subcommand("qq-official", |subcommand| {
            subcommand.about(t(
                "Send text, images, or files through QQ Bot OpenAPI",
                "通过 QQ 官方机器人 OpenAPI 发送文本、图片或文件",
            ))
        })
        .mut_subcommand("qq-bot", |subcommand| {
            subcommand.about(t(
                "Receive QQ Bot events through websocket by default, invoke Sai, and reply through QQ Bot OpenAPI",
                "默认通过 WebSocket 接收 QQ 官方机器人事件，调用 Sai 后通过 QQ Bot OpenAPI 回复",
            ))
        })
        .mut_subcommand("qq-bot-webhook", |subcommand| {
            subcommand.about(t(
                "Receive QQ Bot webhook events through the legacy HTTP callback mode",
                "通过旧 HTTP 回调模式接收 QQ 官方机器人 Webhook 事件",
            ))
        })
        .mut_subcommand("onebot", |subcommand| {
            subcommand.about(t(
                "Send text, images, or files through a OneBot HTTP gateway such as NapCat",
                "通过 NapCat 等 OneBot HTTP 网关发送文本、图片或文件",
            ))
        })
        .mut_subcommand("onebot-server", |subcommand| {
            subcommand.about(t(
                "Receive OneBot HTTP events, invoke Sai, and reply through OneBot",
                "接收 OneBot HTTP 事件，调用 Sai 后通过 OneBot 回复",
            ))
        })
        .mut_subcommand("weixin-login", |subcommand| {
            subcommand.about(t(
                "Log in to Weixin iLink by QR code and save the bot token",
                "通过二维码登录微信 iLink 并保存机器人 token",
            ))
        })
        .mut_subcommand("weixin-server", |subcommand| {
            subcommand.about(t(
                "Receive Weixin iLink long-poll events by token or saved account",
                "通过 token 或已保存账号接收微信 iLink 长轮询事件",
            ))
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
    command.mut_arg("scope", |arg| {
        arg.help(t(
            "all also clears long-term memory",
            "all 同时清空长期记忆",
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
