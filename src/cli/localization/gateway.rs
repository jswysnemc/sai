use crate::i18n::text as t;

/// 本地化网关子命令和参数。
///
/// 参数:
/// - `command`: Clap 网关命令
///
/// 返回:
/// - 已本地化的网关命令
pub(super) fn localize_gateway_command(command: clap::Command) -> clap::Command {
    command
        .mut_arg("verbose", |arg| {
            arg.help(t("Show verbose gateway logs", "显示详细网关日志"))
        })
        .mut_subcommand("start", |subcommand| {
            subcommand.about(t(
                "Start all enabled gateway channels from configuration",
                "启动配置中已启用的所有渠道网关",
            ))
        })
        .mut_subcommand("wecom-webhook", localize_wecom_webhook)
        .mut_subcommand("qq-official", localize_qq_official)
        .mut_subcommand("qq-bot", |subcommand| {
            localize_qq_bot(subcommand.about(t(
                "Receive QQ Bot events through WebSocket by default, invoke Sai, and reply through QQ Bot OpenAPI",
                "默认通过 WebSocket 接收 QQ 官方机器人事件，调用 Sai 后通过 QQ Bot OpenAPI 回复",
            )))
        })
        .mut_subcommand("qq-bot-webhook", |subcommand| {
            localize_qq_bot_credentials(
                subcommand.about(t(
                    "Receive QQ Bot webhook events through the legacy HTTP callback mode",
                    "通过旧 HTTP 回调模式接收 QQ 官方机器人 Webhook 事件",
                )),
            )
        })
        .mut_subcommand("onebot", localize_onebot)
        .mut_subcommand("onebot-server", localize_onebot_server)
        .mut_subcommand("weixin-login", localize_weixin_login)
        .mut_subcommand("weixin-server", localize_weixin_server)
}

/// 本地化通用出站消息参数。
///
/// 参数:
/// - `command`: 含 text、image 和 file 参数的子命令
///
/// 返回:
/// - 已本地化的子命令
fn localize_outbound_message_args(command: clap::Command) -> clap::Command {
    command
        .mut_arg("text", |arg| arg.help(t("Text message", "文本消息")))
        .mut_arg("image", |arg| {
            arg.help(t("Image file path", "图片文件路径"))
        })
        .mut_arg("file", |arg| arg.help(t("File path", "文件路径")))
}

/// 本地化企业微信 Webhook 参数。
fn localize_wecom_webhook(command: clap::Command) -> clap::Command {
    localize_outbound_message_args(
        command
            .about(t(
                "Send text, images, or files through a WeCom group webhook",
                "通过企业微信群机器人 Webhook 发送文本、图片或文件",
            ))
            .mut_arg("webhook_url", |arg| {
                arg.help(t("Full WeCom webhook URL", "企业微信 Webhook 完整地址"))
            }),
    )
}

/// 本地化 QQ 官方机器人出站参数。
fn localize_qq_official(command: clap::Command) -> clap::Command {
    localize_outbound_message_args(
        command
            .about(t(
                "Send text, images, or files through QQ Bot OpenAPI",
                "通过 QQ 官方机器人 OpenAPI 发送文本、图片或文件",
            ))
            .mut_arg("base_url", |arg| {
                arg.help(t("QQ OpenAPI base URL", "QQ OpenAPI 基础地址"))
            })
            .mut_arg("authorization", |arg| {
                arg.help(t("Authorization header value", "Authorization 请求头内容"))
            })
            .mut_arg("target_kind", |arg| {
                arg.help(t("Target kind: user or group", "目标类型：user 或 group"))
            })
            .mut_arg("target_id", |arg| {
                arg.help(t("Target open ID", "目标 Open ID"))
            })
            .mut_arg("msg_id", |arg| {
                arg.help(t("Message ID for passive reply", "被动回复关联消息 ID"))
            }),
    )
}

/// 本地化 QQ 官方机器人入站参数。
fn localize_qq_bot(command: clap::Command) -> clap::Command {
    localize_qq_bot_credentials(command).mut_arg("transport", |arg| {
        arg.help(t(
            "Transport: websocket or webhook",
            "传输模式：websocket 或 webhook",
        ))
    })
}

/// 本地化 QQ 官方机器人通用凭据参数。
fn localize_qq_bot_credentials(command: clap::Command) -> clap::Command {
    command
        .mut_arg("listen", |arg| {
            arg.help(t("Webhook listen address", "Webhook 监听地址"))
        })
        .mut_arg("base_url", |arg| {
            arg.help(t("QQ OpenAPI base URL", "QQ OpenAPI 基础地址"))
        })
        .mut_arg("token", |arg| {
            arg.help(t(
                "QQ token in AppID:AppSecret format",
                "QQ token，格式为 AppID:AppSecret",
            ))
        })
        .mut_arg("app_id", |arg| arg.help(t("QQ App ID", "QQ App ID")))
        .mut_arg("client_secret", |arg| {
            arg.help(t("QQ client secret", "QQ Client Secret"))
        })
}

/// 本地化 OneBot 出站参数。
fn localize_onebot(command: clap::Command) -> clap::Command {
    localize_outbound_message_args(
        command
            .about(t(
                "Send text, images, or files through a OneBot HTTP gateway such as NapCat",
                "通过 NapCat 等 OneBot HTTP 网关发送文本、图片或文件",
            ))
            .mut_arg("base_url", |arg| {
                arg.help(t("OneBot HTTP base URL", "OneBot HTTP 基础地址"))
            })
            .mut_arg("access_token", |arg| {
                arg.help(t("OneBot access token", "OneBot 访问令牌"))
            })
            .mut_arg("target_kind", |arg| {
                arg.help(t(
                    "Target kind: private or group",
                    "目标类型：private 或 group",
                ))
            })
            .mut_arg("target_id", |arg| {
                arg.help(t("Target user or group ID", "目标用户或群组 ID"))
            }),
    )
}

/// 本地化 OneBot 入站服务参数。
fn localize_onebot_server(command: clap::Command) -> clap::Command {
    command
        .about(t(
            "Receive OneBot HTTP events, invoke Sai, and reply through OneBot",
            "接收 OneBot HTTP 事件，调用 Sai 后通过 OneBot 回复",
        ))
        .mut_arg("listen", |arg| {
            arg.help(t("Inbound listen address", "入站监听地址"))
        })
        .mut_arg("onebot_base_url", |arg| {
            arg.help(t("OneBot HTTP base URL", "OneBot HTTP 基础地址"))
        })
        .mut_arg("access_token", |arg| {
            arg.help(t("OneBot access token", "OneBot 访问令牌"))
        })
}

/// 本地化微信登录参数。
fn localize_weixin_login(command: clap::Command) -> clap::Command {
    command
        .about(t(
            "Log in to Weixin iLink by QR code and save the bot token",
            "通过二维码登录微信 iLink 并保存机器人 token",
        ))
        .mut_arg("base_url", |arg| {
            arg.help(t("Weixin iLink base URL", "微信 iLink 基础地址"))
        })
        .mut_arg("bot_type", |arg| {
            arg.help(t("Weixin bot type", "微信机器人类型"))
        })
        .mut_arg("timeout_secs", |arg| {
            arg.help(t("Login timeout in seconds", "登录超时秒数"))
        })
}

/// 本地化微信入站服务参数。
fn localize_weixin_server(command: clap::Command) -> clap::Command {
    command
        .about(t(
            "Receive Weixin iLink long-poll events by token or saved account",
            "通过 token 或已保存账号接收微信 iLink 长轮询事件",
        ))
        .mut_arg("base_url", |arg| {
            arg.help(t("Weixin iLink base URL", "微信 iLink 基础地址"))
        })
        .mut_arg("cdn_base_url", |arg| {
            arg.help(t("Weixin CDN base URL", "微信 CDN 基础地址"))
        })
        .mut_arg("token", |arg| {
            arg.help(t("Weixin bot token", "微信机器人 token"))
        })
        .mut_arg("account", |arg| {
            arg.help(t("Saved Weixin account ID", "已保存微信账号 ID"))
        })
        .mut_arg("bot_agent", |arg| {
            arg.help(t("Weixin bot agent string", "微信 Bot Agent 字符串"))
        })
}
