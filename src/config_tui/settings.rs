use crate::config::AppConfig;
use crate::i18n::text as t;
use anyhow::Result;
use std::io;

use super::form::{
    parse_bool_field, parse_provider_model_choice, provider_model_choice_values, run_form, Field,
};

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
    if run_form(
        stdout,
        t(" GLOBAL SETTINGS ", " 全局参数设置 "),
        &mut fields,
    )? {
        let [tui_mode, cli_mode, terminal_shell, context_tokens, compaction_model, tools_enabled, tool_max_rounds, command_shell, progressive_loading, background_commands, background_timeout, background_log_max, background_stop_grace, skills_enabled, skill_commands, reasoning, tool_calls, readable_names, wait_model, wait_thinking, transcript_rows] =
            fields.as_slice()
        else {
            unreachable!("global settings field layout must remain complete")
        };
        let tui = crate::config::DefaultPermissionMode::parse_or_default(&tui_mode.value);
        let cli = crate::config::DefaultPermissionMode::parse_or_default(&cli_mode.value);
        config.permission.tui_mode = Some(tui);
        config.permission.cli_mode = Some(cli);
        // 兼容旧字段：与 TUI 保持一致。
        config.permission.default_mode = tui;
        config.terminal.shell = terminal_shell.value.trim().to_string();
        config.context.default_max_chars = context_tokens.value.trim().parse::<usize>()?;
        (
            config.context.compaction_provider_id,
            config.context.compaction_model,
        ) = parse_provider_model_choice(&compaction_model.value);
        config.tools.enabled = parse_bool_field(&tools_enabled.value)?;
        config.tools.max_rounds = tool_max_rounds.value.trim().parse::<usize>()?;
        config.tools.command_shell = command_shell.value.trim().to_string();
        config.tools.progressive_loading_enabled = parse_bool_field(&progressive_loading.value)?;
        config.tools.background_commands_enabled = parse_bool_field(&background_commands.value)?;
        config.tools.background_command_timeout_seconds =
            background_timeout.value.trim().parse::<u64>()?;
        config.tools.background_command_log_max_bytes =
            background_log_max.value.trim().parse::<u64>()?;
        config.tools.background_command_stop_grace_seconds =
            background_stop_grace.value.trim().parse::<u64>()?;
        config.skills.enabled = parse_bool_field(&skills_enabled.value)?;
        config.skills.allow_command_execution = parse_bool_field(&skill_commands.value)?;
        config.display.reasoning = reasoning.value.trim().to_string();
        config.display.tool_calls = tool_calls.value.trim().to_string();
        config.display.readable_tool_names = parse_bool_field(&readable_names.value)?;
        config.display.wait_show_model = parse_bool_field(&wait_model.value)?;
        config.display.wait_show_thinking_level = parse_bool_field(&wait_thinking.value)?;
        config.display.repl_transcript_row_cap =
            transcript_rows.value.trim().parse::<usize>()?.max(1);
    }
    Ok(())
}
