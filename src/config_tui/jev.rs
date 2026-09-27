//! Jev 内置功能设置：接入、工具与 skills 暴露决策、权限自动审核与连接测试。

use crate::config::{AppConfig, JevConnectionSource};
use crate::i18n::text as t;
use anyhow::Result;
use crossterm::event::KeyCode;
use std::io;

use super::form::{parse_bool_field, parse_number_field, run_form, Field};
use super::input::read_key;
use super::jev_connection::{edit_connection, probe_blocking, select_connection};
use super::ui::{draw_menu_with_details, message};

/// Jev 设置菜单。
///
/// 参数:
/// - `stdout`: 终端标准输出
/// - `config`: 待更新应用配置
///
/// 返回:
/// - 菜单退出结果
pub(crate) fn edit_jev(stdout: &mut io::Stdout, config: &mut AppConfig) -> Result<()> {
    let mut selected = 0usize;
    loop {
        let options = vec![
            t("Connection", "接入").to_string(),
            t("Edit connection", "编辑接入").to_string(),
            t("Tool & skill routing", "工具与 Skills 暴露决策").to_string(),
            t("Permission audit", "权限自动审核").to_string(),
            t("Test connection", "测试连接").to_string(),
        ];
        let details = vec![
            connection_detail(config),
            t(
                "Edit the endpoint, model and API key of the active JEV connection. Saving creates a connection when none exists.",
                "编辑当前生效 JEV 接入的地址、模型和 API Key；尚无接入时保存会新建一条。",
            )
            .to_string(),
            routing_detail(config),
            audit_detail(config),
            t(
                "Send one minimal request with the saved connection to verify the endpoint, key and response format.",
                "用当前接入发送一次最小请求，验证地址、密钥与响应格式。",
            )
            .to_string(),
        ];
        draw_menu_with_details(
            stdout,
            " JEV ",
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
            KeyCode::Char(digit @ '1'..='5') => selected = digit as usize - '1' as usize,
            KeyCode::Enter => match selected {
                0 => select_connection(stdout, config)?,
                1 => edit_connection(stdout, config)?,
                2 => edit_routing(stdout, config)?,
                3 => edit_audit(stdout, config)?,
                4 => {
                    let text = match config.jev_connection() {
                        Ok(connection) => probe_blocking(connection).summary(),
                        Err(error) => format!("{error:#}"),
                    };
                    message(stdout, &text)?;
                }
                _ => {}
            },
            _ => {}
        }
    }
}

/// 接入菜单的说明：来源、地址与密钥状态。
fn connection_detail(config: &AppConfig) -> String {
    let intro = t(
        "Choose which JEV connection the built-in features use. Without one, the official TypeSafe endpoint and TYPESAFE_API_KEY are used.",
        "选择内置功能使用的 JEV 接入；没有接入时使用 TypeSafe 官方地址和 TYPESAFE_API_KEY。",
    );
    let status = match config.jev_endpoint() {
        Ok(endpoint) => {
            let info = crate::config::jev_connection_info_for(endpoint);
            let source = match info.source {
                JevConnectionSource::Endpoint => info.name,
                JevConnectionSource::Official => t("Official TypeSafe", "TypeSafe 官方").to_string(),
            };
            let key = match crate::config::jev_connection_for(endpoint) {
                Ok(_) => t("key ready", "密钥可用").to_string(),
                Err(error) => format!("{error:#}"),
            };
            format!("{source}\n{}\n{} · {key}", info.endpoint, info.model)
        }
        Err(error) => format!("{error:#}"),
    };
    format!("{intro}\n\n{status}")
}

/// 暴露决策菜单的说明。
fn routing_detail(config: &AppConfig) -> String {
    let routing = &config.jev.routing;
    let mut text = format!(
        "{}\n\n{}: {} · {}: {:.2} · {}: {}/{}",
        t(
            "Before each request Jev picks the tools and skills to expose; the model can ask for more with request_capability. Basic tools are always exposed.",
            "每次请求前由 Jev 选择需要暴露的工具与 Skills，模型可通过 request_capability 追加申请；基础工具始终暴露。",
        ),
        t("Status", "状态"),
        on_off(routing.enabled),
        t("Threshold", "阈值"),
        routing.threshold,
        t("Limits", "上限"),
        routing.max_tools,
        routing.max_skills,
    );
    if routing.enabled && !config.jev_routing_active() {
        text.push_str(t(
            "\n\nDeepSeek anchored mode is active and takes over the tool catalog.",
            "\n\n当前启用了 DeepSeek 锚定模式，工具目录由锚定模式接管。",
        ));
    }
    text
}

/// 审核菜单的说明。
fn audit_detail(config: &AppConfig) -> String {
    let audit = &config.jev.audit;
    format!(
        "{}\n\n{}: {} · p ≥ {:.2} · {} ≥ {:.2}",
        t(
            "In auto-audit permission mode, Jev reviews each pending operation. Uncertain answers and failures go to human review. Takes precedence over audit plugins and the chat model.",
            "自动审核权限模式下由 Jev 审核每个待批操作；判断不确定或调用失败时交还人工。优先于审核插件和聊天模型审核。",
        ),
        t("Status", "状态"),
        on_off(audit.enabled),
        audit.minimum_probability,
        t("confidence", "置信度"),
        audit.minimum_confidence,
    )
}

/// 编辑暴露决策参数。
fn edit_routing(stdout: &mut io::Stdout, config: &mut AppConfig) -> Result<()> {
    let routing = &config.jev.routing;
    let mut fields = vec![
        Field::boolean(t("Enabled", "启用"), routing.enabled),
        Field::new(t("Minimum probability (0-1)", "最低概率（0-1）"), routing.threshold.to_string()),
        Field::new(t("Max tools per decision", "单次最多暴露工具数"), routing.max_tools.to_string()),
        Field::new(t("Max skills per decision", "单次最多暴露 Skills 数"), routing.max_skills.to_string()),
        Field::new(t("Timeout seconds (1-60)", "超时秒数（1-60）"), routing.timeout_seconds.to_string()),
        Field::new(t("Context characters", "判断所用对话字符数"), routing.context_chars.to_string()),
    ];
    loop {
        if !run_form(stdout, t(" JEV ROUTING ", " JEV 暴露决策 "), &mut fields)? {
            return Ok(());
        }
        match apply_routing(config, &fields) {
            Ok(()) => return Ok(()),
            Err(error) => message(stdout, &format!("{}: {error}", t("Invalid input", "输入无效")))?,
        }
    }
}

/// 把暴露决策表单写入配置；任一字段无效时不写入。
fn apply_routing(config: &mut AppConfig, fields: &[Field]) -> Result<()> {
    // 1. 先解析全部字段，再在副本上校验范围
    let mut next = config.jev.clone();
    next.routing.enabled = parse_bool_field(&fields[0].value)?;
    next.routing.threshold = parse_number_field(fields[1].label, &fields[1].value)?;
    next.routing.max_tools = parse_number_field(fields[2].label, &fields[2].value)?;
    next.routing.max_skills = parse_number_field(fields[3].label, &fields[3].value)?;
    next.routing.timeout_seconds = parse_number_field(fields[4].label, &fields[4].value)?;
    next.routing.context_chars = parse_number_field(fields[5].label, &fields[5].value)?;
    next.validate(&config.model_endpoints)?;
    // 2. 全部通过后整体替换
    config.jev = next;
    Ok(())
}

/// 编辑权限自动审核参数。
fn edit_audit(stdout: &mut io::Stdout, config: &mut AppConfig) -> Result<()> {
    let audit = &config.jev.audit;
    let mut fields = vec![
        Field::boolean(t("Enabled", "启用"), audit.enabled),
        Field::new(t("Minimum choice probability (0.5-1)", "最低选项概率（0.5-1）"), audit.minimum_probability.to_string()),
        Field::new(t("Minimum confidence (0.5-1)", "最低置信度（0.5-1）"), audit.minimum_confidence.to_string()),
        Field::new(t("Timeout seconds (1-30)", "超时秒数（1-30）"), audit.timeout_seconds.to_string()),
    ];
    loop {
        if !run_form(stdout, t(" JEV AUDIT ", " JEV 权限审核 "), &mut fields)? {
            return Ok(());
        }
        match apply_audit(config, &fields) {
            Ok(()) => return Ok(()),
            Err(error) => message(stdout, &format!("{}: {error}", t("Invalid input", "输入无效")))?,
        }
    }
}

/// 把审核表单写入配置；任一字段无效时不写入。
fn apply_audit(config: &mut AppConfig, fields: &[Field]) -> Result<()> {
    let mut next = config.jev.clone();
    next.audit.enabled = parse_bool_field(&fields[0].value)?;
    next.audit.minimum_probability = parse_number_field(fields[1].label, &fields[1].value)?;
    next.audit.minimum_confidence = parse_number_field(fields[2].label, &fields[2].value)?;
    next.audit.timeout_seconds = parse_number_field(fields[3].label, &fields[3].value)?;
    next.validate(&config.model_endpoints)?;
    config.jev = next;
    Ok(())
}

/// 开关状态文本。
fn on_off(value: bool) -> &'static str {
    if value {
        t("on", "启用")
    } else {
        t("off", "关闭")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(value: &str) -> Field {
        Field::new("field", value.to_string())
    }

    #[test]
    fn invalid_routing_input_keeps_previous_config() {
        let mut config = AppConfig::default();
        let fields = ["true", "1.5", "4", "2", "20", "2000"].map(field);
        assert!(apply_routing(&mut config, &fields).is_err());
        assert!(!config.jev.routing.enabled);
    }

    #[test]
    fn valid_audit_input_is_applied() {
        let mut config = AppConfig::default();
        let fields = ["true", "0.95", "0.85", "8"].map(field);
        apply_audit(&mut config, &fields).unwrap();
        assert!(config.jev.audit.enabled);
        assert_eq!(config.jev.audit.minimum_probability, 0.95);
        assert_eq!(config.jev.audit.timeout_seconds, 8);
    }
}
