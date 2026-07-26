use crate::config::AppConfig;
use crate::i18n::text as t;
use anyhow::Result;
use std::io;

use super::form::{
    parse_bool_field, parse_number_field, parse_provider_model_choice,
    provider_model_choice_values, run_form, Field,
};
use super::ui::message;

/// 编辑 CLI 与 TUI 共用的运行、权限和显示设置。
///
/// 参数:
/// - `stdout`: 终端标准输出
/// - `config`: 待更新应用配置
///
/// 返回:
/// - 表单退出或保存结果
pub(crate) fn edit_settings(stdout: &mut io::Stdout, config: &mut AppConfig) -> Result<()> {
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
        Field::boolean(t("Tools enabled", "工具启用"), config.tools.enabled),
        Field::new(
            t("Tool max rounds", "工具最大轮数"),
            config.tools.max_rounds.to_string(),
        ),
        Field::new(
            t(
                "Command shell, empty uses user shell",
                "命令执行 Shell，留空使用用户 Shell",
            ),
            config.tools.command_shell.clone(),
        ),
        Field::new(
            t(
                "Command output filter (rtk proxy)",
                "命令输出过滤器（rtk 代理）",
            ),
            config.tools.command_filter.clone(),
        )
        .choices(&["auto", "rtk", "off"]),
        Field::boolean(
            t("Progressive tool loading", "渐进式工具加载"),
            config.tools.progressive_loading_enabled,
        ),
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
        Field::boolean(t("Skills enabled", "Skills 启用"), config.skills.enabled),
        Field::boolean(
            t("Allow command execution", "允许执行命令"),
            config.skills.allow_command_execution,
        ),
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
        if !run_form(
            stdout,
            t(" GLOBAL SETTINGS ", " 全局参数设置 "),
            &mut fields,
        )? {
            return Ok(());
        }
        // 解析失败时就地提示并重新打开表单，不让非法输入终止 TUI
        match apply_settings_fields(config, &fields) {
            Ok(()) => return Ok(()),
            Err(err) => message(stdout, &format!("{}: {err}", t("Invalid input", "输入无效")))?,
        }
    }
}

/// 将全局设置表单值写入应用配置。
///
/// 参数:
/// - `config`: 待更新应用配置
/// - `fields`: 表单字段
///
/// 返回:
/// - 全部字段解析成功时写入配置；否则返回首个解析错误
fn apply_settings_fields(config: &mut AppConfig, fields: &[Field]) -> Result<()> {
    let [tui_mode, cli_mode, terminal_shell, context_tokens, compaction_model, tools_enabled, tool_max_rounds, command_shell, command_filter, progressive_loading, background_commands, background_timeout, background_log_max, background_stop_grace, skills_enabled, skill_commands, reasoning, tool_calls, readable_names, wait_model, wait_thinking, transcript_rows] =
        fields
    else {
        unreachable!("global settings field layout must remain complete")
    };
    // 1. 先完成全部易失败的解析，避免半写入配置
    let default_max_chars =
        parse_number_field::<usize>(context_tokens.label, &context_tokens.value)?;
    let max_rounds = parse_number_field::<usize>(tool_max_rounds.label, &tool_max_rounds.value)?;
    let timeout_seconds =
        parse_number_field::<u64>(background_timeout.label, &background_timeout.value)?;
    let log_max_bytes =
        parse_number_field::<u64>(background_log_max.label, &background_log_max.value)?;
    let stop_grace_seconds =
        parse_number_field::<u64>(background_stop_grace.label, &background_stop_grace.value)?;
    let transcript_row_cap =
        parse_number_field::<usize>(transcript_rows.label, &transcript_rows.value)?;
    let tools_enabled = parse_bool_field(&tools_enabled.value)?;
    let progressive_loading = parse_bool_field(&progressive_loading.value)?;
    let background_commands = parse_bool_field(&background_commands.value)?;
    let skills_enabled = parse_bool_field(&skills_enabled.value)?;
    let skill_commands = parse_bool_field(&skill_commands.value)?;
    let readable_names = parse_bool_field(&readable_names.value)?;
    let wait_model = parse_bool_field(&wait_model.value)?;
    let wait_thinking = parse_bool_field(&wait_thinking.value)?;
    // 2. 解析全部通过后统一写入配置
    let tui = crate::config::DefaultPermissionMode::parse_or_default(&tui_mode.value);
    let cli = crate::config::DefaultPermissionMode::parse_or_default(&cli_mode.value);
    config.permission.tui_mode = Some(tui);
    config.permission.cli_mode = Some(cli);
    // 3. 兼容旧字段，与 TUI 保持一致
    config.permission.default_mode = tui;
    config.terminal.shell = terminal_shell.value.trim().to_string();
    config.context.default_max_chars = default_max_chars;
    (
        config.context.compaction_provider_id,
        config.context.compaction_model,
    ) = parse_provider_model_choice(&compaction_model.value);
    config.tools.enabled = tools_enabled;
    config.tools.max_rounds = max_rounds;
    config.tools.command_shell = command_shell.value.trim().to_string();
    config.tools.command_filter = command_filter.value.trim().to_string();
    config.tools.progressive_loading_enabled = progressive_loading;
    config.tools.background_commands_enabled = background_commands;
    config.tools.background_command_timeout_seconds = timeout_seconds;
    config.tools.background_command_log_max_bytes = log_max_bytes;
    config.tools.background_command_stop_grace_seconds = stop_grace_seconds;
    config.skills.enabled = skills_enabled;
    config.skills.allow_command_execution = skill_commands;
    config.display.reasoning = reasoning.value.trim().to_string();
    config.display.tool_calls = tool_calls.value.trim().to_string();
    config.display.readable_tool_names = readable_names;
    config.display.wait_show_model = wait_model;
    config.display.wait_show_thinking_level = wait_thinking;
    config.display.repl_transcript_row_cap = transcript_row_cap.max(1);
    Ok(())
}
