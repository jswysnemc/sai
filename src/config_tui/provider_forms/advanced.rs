use super::values::*;
use super::*;
use crate::config_tui::form::parse_number_field;

/// 【服务商配置】【高级选项】编辑请求行为，全部校验成功后更新草稿。
/// 参数: stdout 为终端，provider 为草稿；返回: 表单执行结果
pub(super) fn edit(stdout: &mut io::Stdout, provider: &mut ProviderConfig) -> Result<()> {
    let mut fields = vec![
        Field::new(
            t("Timeout seconds", "超时秒数"),
            provider.timeout_seconds.to_string(),
        ),
        Field::new(
            t("Temperature", "温度参数"),
            provider
                .temperature
                .map(|value| value.to_string())
                .unwrap_or_default(),
        ),
        Field::new(
            t("Thinking level", "思考等级"),
            provider.thinking_level.clone(),
        )
        .choices(&["auto", "none", "low", "medium", "high", "xhigh", "max"]),
        Field::new(
            t("Thinking format", "思考格式"),
            provider.thinking_format.clone(),
        )
        .choices(&[
            "auto",
            "string",
            "object",
            "deepseek-thinking",
            "moonshot-thinking",
            "openai-chat-reasoning-effort",
            "reasoning",
            "anthropic-thinking",
            "disabled",
        ]),
        Field::textarea(
            t("Custom Body JSON", "自定义 Body JSON"),
            provider.extra_body.clone(),
        ),
        Field::new(
            t("Client style", "客户端模拟"),
            provider.client_style.clone(),
        )
        .choices(&["auto", "default", "codex", "claude"]),
        Field::boolean(
            t("Claude 1M context", "Claude 启用 1M 上下文"),
            provider.claude_1m_context,
        ),
        Field::new(
            t("Claude maximum output", "Claude 最大输出"),
            provider.anthropic_max_tokens.to_string(),
        ),
        Field::new("User-Agent", provider.user_agent.clone()),
        Field::textarea(
            t("Extra headers JSON", "自定义请求头 JSON"),
            serde_json::to_string_pretty(&provider.extra_headers)?,
        ),
    ];
    loop {
        if !run_form(
            stdout,
            t(" ADVANCED REQUEST ", " 高级请求设置 "),
            &mut fields,
        )? {
            return Ok(());
        }
        match apply(provider, &fields) {
            Ok(next) => {
                *provider = next;
                return Ok(());
            }
            Err(error) => message(
                stdout,
                &format!("{}: {error}", t("Invalid input", "输入无效")),
            )?,
        }
    }
}

/// 【服务商配置】【参数校验】在副本上解析高级字段，避免部分写入。
/// 参数: provider 为原配置，fields 为表单；返回: 完整新配置或错误
fn apply(provider: &ProviderConfig, fields: &[Field]) -> Result<ProviderConfig> {
    let mut next = provider.clone();
    next.timeout_seconds = parse_number_field(fields[0].label, &fields[0].value)?;
    if next.timeout_seconds == 0 {
        bail!(t("Timeout must be positive", "超时秒数必须大于零"));
    }
    next.temperature = if fields[1].value.trim().is_empty() {
        None
    } else {
        let value: f64 = parse_number_field(fields[1].label, &fields[1].value)?;
        if !value.is_finite() || !(0.0..=2.0).contains(&value) {
            bail!(t("Temperature must be 0-2", "温度参数必须介于 0–2"));
        }
        Some(value)
    };
    next.thinking_level = fields[2].value.clone();
    next.thinking_format = fields[3].value.clone();
    next.extra_body = normalize_extra_body(&fields[4].value)?;
    next.client_style = fields[5].value.clone();
    next.claude_1m_context = parse_bool_field(&fields[6].value)?;
    next.anthropic_max_tokens = parse_number_field(fields[7].label, &fields[7].value)?;
    next.user_agent = fields[8].value.trim().into();
    next.extra_headers = normalize_extra_headers(&fields[9].value)?;
    Ok(next)
}
