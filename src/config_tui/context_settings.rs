use super::form::{
    parse_bool_field, parse_number_field, parse_provider_model_choice,
    provider_model_choice_values, run_form, Field,
};
use super::ui::message;
use crate::config::{AppConfig, PasteImageKey};
use crate::i18n::text as t;
use anyhow::Result;
use std::io;

/// 【上下文】【配置编辑】编辑终端、上下文预算、压缩模型与实验开关
/// 参数: stdout 为终端输出，config 为待保存配置；返回编辑结果
pub(super) fn edit_context_settings(stdout: &mut io::Stdout, config: &mut AppConfig) -> Result<()> {
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
                "Clipboard paste key (ctrl_v, alt_v, both)",
                "剪贴板粘贴键（ctrl_v、alt_v、both）",
            ),
            config.input.paste_image_key.as_str().to_string(),
        )
        .choices(&["ctrl_v", "alt_v", "both"]),
        Field::boolean(
            t("Experimental tool-result compression", "实验性工具结果压缩"),
            config.context.experimental_context_blocks,
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
        let experimental = parse_bool_field(&fields[4].value)?;
        config.context.experimental_context_blocks = experimental;
        config.terminal.shell = fields[0].value.trim().to_string();
        config.context.default_max_chars = default_max_chars;
        (
            config.context.compaction_provider_id,
            config.context.compaction_model,
        ) = parse_provider_model_choice(&fields[2].value);
        // 无法识别的键位退回平台默认，而不是让输入框彻底失去粘贴能力
        config.input.paste_image_key = PasteImageKey::parse(&fields[3].value).unwrap_or_default();
        return Ok(());
    }
}
