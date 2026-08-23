//! 全局参数设置：按主题拆成小节表单，避免单个巨型表单里翻找。
//!
//! Skills 全局开关已迁至主菜单 Skills 管理页。

use crate::config::AppConfig;
use crate::i18n::text as t;
use anyhow::Result;
use crossterm::event::KeyCode;
use std::io;

use super::form::{
    parse_bool_field, parse_number_field, parse_provider_model_choice,
    provider_model_choice_values, run_form, Field,
};
use super::input::read_key;
use super::ui::{draw_menu_with_details, message};

/// 全局参数小节菜单。
///
/// 参数:
/// - `stdout`: 终端标准输出
/// - `config`: 待更新应用配置
///
/// 返回:
/// - 菜单退出结果
pub(crate) fn edit_settings(stdout: &mut io::Stdout, config: &mut AppConfig) -> Result<()> {
    let mut selected = 0usize;
    loop {
        let options = vec![
            t("Permissions", "权限").to_string(),
            t("Terminal & context", "终端与上下文").to_string(),
            t("Tools & background commands", "工具与后台命令").to_string(),
            t("Display", "显示偏好").to_string(),
        ];
        let details = vec![
            format!(
                "{}\n\nTUI: {} · CLI: {}",
                t(
                    "Default permission mode for TUI and CLI sessions.",
                    "TUI 与 CLI 会话的默认权限模式。",
                ),
                config.permission.tui_mode().as_str(),
                config.permission.cli_mode().as_str(),
            ),
            format!(
                "{}\n\n{}: {} · {}: {}",
                t(
                    "Web terminal shell, context budget, compact ratio and reserve.",
                    "网页终端 Shell、上下文预算、压缩比例与预留。",
                ),
                t("Context chars", "上下文字符"),
                config.context.default_max_chars,
                t("Compact", "压缩"),
                format!(
                    "{} · {}% · {} {}",
                    compaction_label(config),
                    (config.context.clamped_compaction_ratio() * 100.0).round() as u32,
                    t("reserve", "预留"),
                    config.context.compaction_reserve_tokens
                ),
            ),
            format!(
                "{}\n\n{}: {} · {}: {}",
                t(
                    "Tool availability, command shell, output filter and background command limits.",
                    "工具可用性、命令 Shell、输出过滤器与后台命令限制。",
                ),
                t("Tools", "工具"),
                on_off(config.tools.enabled),
                t("Background", "后台命令"),
                on_off(config.tools.background_commands_enabled),
            ),
            format!(
                "{}\n\n{}: {} · {}: {}",
                t(
                    "Reasoning and tool call visibility, wait animation and REPL replay.",
                    "思考过程与工具调用显示、等待动效与 REPL 重放。",
                ),
                t("Reasoning", "思考过程"),
                config.display.reasoning,
                t("Tool calls", "工具调用"),
                config.display.tool_calls,
            ),
        ];
        draw_menu_with_details(
            stdout,
            t(" GLOBAL PARAMETERS ", " 全局参数 "),
            &options,
            &details,
            selected,
            &super::theme::help_line(&[
                ("↑↓", t("move", "移动")),
                ("Enter", t("open", "打开")),
                ("q", t("back", "返回")),
            ]),
            "",
        )?;
        match read_key()? {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(()),
            KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => selected = (selected + 1).min(options.len() - 1),
            KeyCode::Char(digit @ '1'..='4') => {
                selected = digit as usize - '1' as usize;
            }
            KeyCode::Enter => match selected {
                0 => edit_permission_settings(stdout, config)?,
                1 => edit_context_settings(stdout, config)?,
                2 => edit_tool_settings(stdout, config)?,
                3 => edit_display_settings(stdout, config)?,
                _ => {}
            },
            _ => {}
        }
    }
}

/// 压缩模型的展示标签。
fn compaction_label(config: &AppConfig) -> String {
    if config.context.compaction_provider_id.is_empty()
        || config.context.compaction_model.is_empty()
    {
        t("follow conversation model", "沿用会话模型").to_string()
    } else {
        format!(
            "{}/{}",
            config.context.compaction_provider_id, config.context.compaction_model
        )
    }
}

fn on_off(value: bool) -> &'static str {
    if value {
        t("on", "启用")
    } else {
        t("off", "关闭")
    }
}

/// 编辑 TUI 与 CLI 默认权限模式。
fn edit_permission_settings(stdout: &mut io::Stdout, config: &mut AppConfig) -> Result<()> {
    let mut fields = vec![
        Field::new(
            t("TUI default permission mode", "TUI 默认权限模式"),
            config.permission.tui_mode().as_str().to_string(),
        )
        .choices(&["yolo", "audited", "plan"]),
        Field::new(
            t("CLI default permission mode", "CLI 默认权限模式"),
            config.permission.cli_mode().as_str().to_string(),
        )
        .choices(&["yolo", "audited", "plan"]),
    ];
    if !run_form(stdout, t(" PERMISSIONS ", " 权限 "), &mut fields)? {
        return Ok(());
    }
    let tui = crate::config::DefaultPermissionMode::parse_or_default(&fields[0].value);
    let cli = crate::config::DefaultPermissionMode::parse_or_default(&fields[1].value);
    config.permission.tui_mode = Some(tui);
    config.permission.cli_mode = Some(cli);
    // 兼容旧字段，与 TUI 保持一致
    config.permission.default_mode = tui;
    Ok(())
}

/// 编辑终端 Shell、上下文预算与压缩模型。
fn edit_context_settings(stdout: &mut io::Stdout, config: &mut AppConfig) -> Result<()> {
    let mut fields = vec![
        Field::new(
            t("Web terminal shell", "网页终端 Shell"),
            config.terminal.shell.clone(),
        ),
        Field::new(
            t("Default context characters", "默认上下文字符数"),
            config.context.default_max_chars.to_string(),
        ),
        Field::new(
            t("Compaction provider/model", "压缩供应商/模型"),
            if config.context.compaction_provider_id.is_empty()
                || config.context.compaction_model.is_empty()
            {
                String::new()
            } else {
                format!(
                    "{}\t{}",
                    config.context.compaction_provider_id, config.context.compaction_model
                )
            },
        )
        .choices_owned(provider_model_choice_values(config, false))
        .empty_choice_label(t("Follow conversation model", "沿用会话模型")),
        Field::new(
            t(
                "Auto-compact ratio, 0.50-0.99 or 50-99",
                "自动压缩比例，0.50-0.99 或 50-99",
            ),
            format!(
                "{}",
                (config.context.clamped_compaction_ratio() * 100.0).round() as u32
            ),
        ),
        Field::new(
            t(
                "Reserved tokens, 0 uses ratio only",
                "压缩预留 token，0 表示只按比例",
            ),
            config.context.compaction_reserve_tokens.to_string(),
        ),
    ];
    loop {
        if !run_form(
            stdout,
            t(" TERMINAL & CONTEXT ", " 终端与上下文 "),
            &mut fields,
        )? {
            return Ok(());
        }
        // 解析失败时就地提示并重新打开表单，不让非法输入终止 TUI
        let default_max_chars = match parse_number_field::<usize>(fields[1].label, &fields[1].value)
        {
            Ok(value) => value,
            Err(err) => {
                message(
                    stdout,
                    &format!("{}: {err}", t("Invalid input", "输入无效")),
                )?;
                continue;
            }
        };
        let compaction_ratio = match crate::config::parse_compaction_ratio_text(&fields[3].value) {
            Ok(value) => value,
            Err(err) => {
                message(
                    stdout,
                    &format!("{}: {err}", t("Invalid input", "输入无效")),
                )?;
                continue;
            }
        };
        let compaction_reserve_tokens =
            match parse_number_field::<usize>(fields[4].label, &fields[4].value) {
                Ok(value) => value,
                Err(err) => {
                    message(
                        stdout,
                        &format!("{}: {err}", t("Invalid input", "输入无效")),
                    )?;
                    continue;
                }
            };
        config.terminal.shell = fields[0].value.trim().to_string();
        config.context.default_max_chars = default_max_chars;
        (
            config.context.compaction_provider_id,
            config.context.compaction_model,
        ) = parse_provider_model_choice(&fields[2].value);
        config.context.compaction_ratio = compaction_ratio;
        config.context.compaction_reserve_tokens = compaction_reserve_tokens;
        return Ok(());
    }
}

/// 编辑工具行为与后台命令限制。
fn edit_tool_settings(stdout: &mut io::Stdout, config: &mut AppConfig) -> Result<()> {
    let mut fields = vec![
        Field::section(t("Tools", "工具")),
        Field::boolean(t("Tools enabled", "工具启用"), config.tools.enabled),
        Field::new(
            t(
                "Command shell, empty uses user shell",
                "命令执行 Shell，留空使用用户 Shell",
            ),
            config.tools.command_shell.clone(),
        ),
        Field::new(
            // 标签同时展示 rtk 探测状态，便于判断 auto 档位是否会生效
            if crate::tools::command::rtk_available() {
                t(
                    "Command output filter (rtk detected)",
                    "命令输出过滤器（已检测到 rtk）",
                )
            } else {
                t(
                    "Command output filter (rtk not installed)",
                    "命令输出过滤器（未检测到 rtk）",
                )
            },
            config.tools.command_filter.clone(),
        )
        .choices(&["auto", "rtk", "off"]),
        {
            // 表单字段是单行且会按终端宽度截断，rtk 能代理的命令有数十项塞不下；
            // 这里只给出可代理的条目数，完整清单在 Web 设置页展示。label 需 'static，缓存一次
            static DENYLIST_LABEL: std::sync::OnceLock<String> = std::sync::OnceLock::new();
            let label = DENYLIST_LABEL.get_or_init(|| {
                let count = crate::tools::command::rtk_proxy_commands().len();
                if crate::i18n::is_zh() {
                    format!("不走 rtk 的命令（rtk 现可代理 {count} 项），逗号分隔")
                } else {
                    format!("commands kept out of rtk (rtk proxies {count}), comma separated")
                }
            });
            Field::new(label, config.tools.command_filter_denylist.join(", "))
        },
        Field::section(t("Background commands", "后台命令")),
        Field::boolean(
            t("Background commands enabled", "后台命令启用"),
            config.tools.background_commands_enabled,
        ),
        Field::new(
            t(
                "Background command default timeout seconds, 0 means no timeout",
                "后台命令默认超时秒数，0 表示不超时",
            ),
            config.tools.background_command_timeout_seconds.to_string(),
        ),
        Field::new(
            t("Background command max log bytes", "后台命令日志最大字节"),
            config.tools.background_command_log_max_bytes.to_string(),
        ),
        Field::new(
            t(
                "Background command stop grace seconds",
                "后台命令停止宽限秒数",
            ),
            config
                .tools
                .background_command_stop_grace_seconds
                .to_string(),
        ),
    ];
    loop {
        if !run_form(
            stdout,
            t(" TOOLS & BACKGROUND ", " 工具与后台命令 "),
            &mut fields,
        )? {
            return Ok(());
        }
        match apply_tool_fields(config, &fields) {
            Ok(()) => return Ok(()),
            Err(err) => message(
                stdout,
                &format!("{}: {err}", t("Invalid input", "输入无效")),
            )?,
        }
    }
}

/// 将工具小节表单值写入配置；任一字段解析失败时不写入。
fn apply_tool_fields(config: &mut AppConfig, fields: &[Field]) -> Result<()> {
    // 分组标题行只用于视觉分区，取值前先剔除
    let values: Vec<&Field> = fields.iter().filter(|field| !field.section).collect();
    let [tools_enabled, command_shell, command_filter, command_filter_denylist, background_commands, background_timeout, background_log_max, background_stop_grace] =
        values.as_slice()
    else {
        unreachable!("tool settings field layout must remain complete")
    };
    // 1. 先完成全部易失败的解析，避免半写入配置
    let timeout_seconds =
        parse_number_field::<u64>(background_timeout.label, &background_timeout.value)?;
    let log_max_bytes =
        parse_number_field::<u64>(background_log_max.label, &background_log_max.value)?;
    let stop_grace_seconds =
        parse_number_field::<u64>(background_stop_grace.label, &background_stop_grace.value)?;
    let tools_enabled = parse_bool_field(&tools_enabled.value)?;
    let background_commands = parse_bool_field(&background_commands.value)?;
    // 2. 解析全部通过后统一写入配置
    config.tools.enabled = tools_enabled;
    config.tools.command_shell = command_shell.value.trim().to_string();
    config.tools.command_filter = command_filter.value.trim().to_string();
    config.tools.command_filter_denylist = command_filter_denylist
        .value
        .split(',')
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect();
    config.tools.background_commands_enabled = background_commands;
    config.tools.background_command_timeout_seconds = timeout_seconds;
    config.tools.background_command_log_max_bytes = log_max_bytes;
    config.tools.background_command_stop_grace_seconds = stop_grace_seconds;
    Ok(())
}

/// 编辑显示偏好。
fn edit_display_settings(stdout: &mut io::Stdout, config: &mut AppConfig) -> Result<()> {
    let mut fields = vec![
        Field::new(
            t("Show reasoning", "显示思考过程"),
            config.display.reasoning.clone(),
        )
        .choices(&["summary", "full", "hidden"]),
        Field::new(
            t("Show tool call information", "显示工具调用信息"),
            config.display.tool_calls.clone(),
        )
        .choices(&["summary", "full", "hidden"]),
        Field::boolean(
            t("Readable tool names", "工具名可读显示"),
            config.display.readable_tool_names,
        ),
        Field::boolean(
            t("Show model in wait animation", "等待动效显示模型"),
            config.display.wait_show_model,
        ),
        Field::boolean(
            t(
                "Show thinking level in wait animation",
                "等待动效显示思考等级",
            ),
            config.display.wait_show_thinking_level,
        ),
        Field::new(
            t("REPL transcript row cap", "REPL 历史重放行数上限"),
            config.display.repl_transcript_row_cap.to_string(),
        ),
    ];
    loop {
        if !run_form(stdout, t(" DISPLAY ", " 显示偏好 "), &mut fields)? {
            return Ok(());
        }
        match apply_display_fields(config, &fields) {
            Ok(()) => return Ok(()),
            Err(err) => message(
                stdout,
                &format!("{}: {err}", t("Invalid input", "输入无效")),
            )?,
        }
    }
}

/// 将显示小节表单值写入配置；任一字段解析失败时不写入。
fn apply_display_fields(config: &mut AppConfig, fields: &[Field]) -> Result<()> {
    let transcript_row_cap = parse_number_field::<usize>(fields[5].label, &fields[5].value)?;
    let readable_names = parse_bool_field(&fields[2].value)?;
    let wait_model = parse_bool_field(&fields[3].value)?;
    let wait_thinking = parse_bool_field(&fields[4].value)?;
    config.display.reasoning = fields[0].value.trim().to_string();
    config.display.tool_calls = fields[1].value.trim().to_string();
    config.display.readable_tool_names = readable_names;
    config.display.wait_show_model = wait_model;
    config.display.wait_show_thinking_level = wait_thinking;
    config.display.repl_transcript_row_cap = transcript_row_cap.max(1);
    Ok(())
}
