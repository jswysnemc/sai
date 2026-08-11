use crate::config::ProviderConfig;
use crate::i18n::text as t;
use anyhow::{bail, Result};
use serde_json::Value;
use std::io;

use super::form::{parse_bool_field, run_form, Field};
use super::input::read_key;
use super::model_metadata_form::{
    apply_context_chars_field, apply_max_output_tokens_field, apply_tag_fields,
    apply_tools_enabled_field, apply_web_search_tool_mode_field, context_chars_field_value,
    max_output_tokens_field_value, tag_fields, tools_enabled_field, web_search_tool_mode_field,
};
use super::ui::{draw_menu, message};

/// 编辑 provider 配置表单。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `provider`: 原 provider 配置
///
/// 返回:
/// - 修改后的 provider 配置，取消时返回空
pub(super) fn edit_provider_form(
    stdout: &mut io::Stdout,
    provider: ProviderConfig,
) -> Result<Option<ProviderConfig>> {
    let claude_simulation = is_claude_client_style(&provider.client_style);
    let mut fields = vec![
        Field::new(t("Config ID", "配置 ID"), provider.id.clone()),
        Field::new(t("Display name", "显示名称"), provider.display_name.clone()),
        Field::new(t("Base URL", "基础地址"), provider.base_url.clone()),
        Field::new(t("Protocol", "协议"), provider.protocol.clone()).choices(&[
            "auto",
            "openai-chat",
            "openai-responses",
            "anthropic",
        ]),
        Field::new(
            "API Key or $env:NAME",
            provider.api_key.clone().unwrap_or_default(),
        )
        .secret(),
        Field::new(
            t("Timeout seconds", "超时秒数"),
            provider.timeout_seconds.to_string(),
        ),
        Field::new(
            t("Temperature", "温度参数"),
            provider.temperature.to_string(),
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
    ];
    // 1. Claude 模拟专用字段仅在当前已是 Claude 时展示
    if claude_simulation {
        fields.push(
            Field::new(
                t("Claude 1M context", "Claude 启用 1M 上下文"),
                if provider.claude_1m_context {
                    "true".to_string()
                } else {
                    "false".to_string()
                },
            )
            .choices(&["true", "false"]),
        );
        fields.push(Field::new(
            t("Claude max output", "Claude 最大输出"),
            provider.anthropic_max_tokens.to_string(),
        ));
    }
    fields.push(Field::new(
        t("User-Agent", "User-Agent"),
        provider.user_agent.clone(),
    ));
    fields.push(Field::textarea(
        t("Extra Headers JSON", "自定义请求头 JSON"),
        if provider.extra_headers.is_empty() {
            String::new()
        } else {
            serde_json::to_string_pretty(&provider.extra_headers).unwrap_or_default()
        },
    ));
    // 多密钥管理：每行一个密钥，可在竖线后附备注；非空时优先于上面的单密钥字段。
    // 注意：此处不能用 secret 掩码——textarea 的 value 会原样写回，掩码串将覆盖真实密钥。
    fields.push(Field::textarea(
        t(
            "API keys (one per line; optional label after ' | ')",
            "接口密钥（每行一个，竖线后可加备注）",
        ),
        render_api_key_lines(&provider.api_keys),
    ));
    fields.push(
        Field::new(
            t("Balance API keys", "在密钥间负载均衡"),
            provider.api_key_balance.to_string(),
        )
        .choices(&["true", "false"]),
    );
    fields.push(Field::new(
        t(
            "Selected key (1-based; blank = first)",
            "选用密钥序号（从 1 起，空为第一个）",
        ),
        render_selected_index(&provider),
    ));
    loop {
        if !run_form(stdout, t(" EDIT PROVIDER ", " 编辑供应商 "), &mut fields)? {
            return Ok(None);
        }
        // 校验失败时就地提示并重新打开表单，不让非法输入终止 TUI
        match build_provider_from_fields(&provider, claude_simulation, &fields) {
            Ok(updated) => return Ok(Some(updated)),
            Err(err) => message(
                stdout,
                &format!("{}: {err}", t("Invalid input", "输入无效")),
            )?,
        }
    }
}

/// 根据表单字段组装 provider 配置。
///
/// 参数:
/// - `provider`: 原 provider 配置
/// - `claude_simulation`: 打开表单时是否展示 Claude 专用字段
/// - `fields`: 表单字段
///
/// 返回:
/// - 组装后的 provider 配置；JSON 或布尔字段非法时返回错误
fn build_provider_from_fields(
    provider: &ProviderConfig,
    claude_simulation: bool,
    fields: &[Field],
) -> Result<ProviderConfig> {
    let extra_body = normalize_extra_body(&fields[9].value)?;
    let client_style = fields[10].value.trim().to_string();
    let (claude_1m_context, anthropic_max_tokens, user_agent_idx, headers_idx) =
        if claude_simulation {
            // 字段顺序：… client_style, 1m, max_output, user_agent, headers
            (
                parse_bool_field(&fields[11].value)?,
                fields[12]
                    .value
                    .trim()
                    .parse()
                    .unwrap_or(provider.anthropic_max_tokens),
                13usize,
                14usize,
            )
        } else {
            // 未展示 Claude 字段时保留原值；若本次切到 claude 则 1M 默认 true
            let next_claude = is_claude_client_style(&client_style);
            (
                if next_claude {
                    true
                } else {
                    provider.claude_1m_context
                },
                provider.anthropic_max_tokens,
                11usize,
                12usize,
            )
        };
    let extra_headers = normalize_extra_headers(&fields[headers_idx].value)?;
    // 多密钥三字段固定在表单末尾：密钥列表、负载均衡开关、选中序号
    let api_keys = parse_api_key_lines(&fields[fields.len() - 3].value, &provider.api_keys);
    let api_key_balance = parse_bool_field(&fields[fields.len() - 2].value)?;
    let api_key_selected = parse_selected_key(&fields[fields.len() - 1].value, &api_keys);
    let updated = ProviderConfig {
        id: fields[0].value.trim().to_string(),
        display_name: fields[1].value.trim().to_string(),
        base_url: normalize_base_url(&fields[2].value),
        // 启用开关不在 TUI 表单里，沿用编辑前的值
        enabled: provider.enabled,
        protocol: fields[3].value.trim().to_string(),
        api_key: Some(fields[4].value.trim().to_string()).filter(|value| !value.is_empty()),
        api_keys,
        api_key_selected,
        api_key_balance,
        models: provider.models.clone(),
        model_context_chars: provider.model_context_chars.clone(),
        model_metadata: provider.model_metadata.clone(),
        default_model: provider.default_model.clone(),
        timeout_seconds: fields[5].value.trim().parse().unwrap_or(60),
        temperature: fields[6].value.trim().parse().unwrap_or(0.7),
        anthropic_max_tokens,
        thinking_level: fields[7].value.trim().to_string(),
        thinking_format: fields[8].value.trim().to_string(),
        preserve_thinking: provider.preserve_thinking,
        extra_body,
        extra_headers,
        user_agent: fields[user_agent_idx].value.trim().to_string(),
        client_style,
        claude_1m_context,
    };
    Ok(updated)
}

/// 判断客户端模拟是否为 Claude Code。
///
/// 参数:
/// - `style`: client_style 字段
///
/// 返回:
/// - Claude 模拟时 true
fn is_claude_client_style(style: &str) -> bool {
    matches!(
        style.trim().to_ascii_lowercase().as_str(),
        "claude" | "claude-code" | "claude_code"
    )
}

/// 规范化并校验自定义 Body JSON。
///
/// 参数:
/// - `value`: 表单中输入的 JSON 文本
///
/// 返回:
/// - 为空时返回空字符串，否则返回格式化后的 JSON 对象字符串
/// 规范化自定义请求头 JSON 对象。
///
/// 参数:
/// - `value`: 表单 JSON 文本
///
/// 返回:
/// - 键值表；空输入返回空表
fn normalize_extra_headers(value: &str) -> Result<std::collections::HashMap<String, String>> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let parsed = serde_json::from_str::<Value>(value)?;
    let obj = parsed.as_object().ok_or_else(|| {
        anyhow::anyhow!(
            "{}",
            t(
                "Extra Headers JSON must be a JSON object of string values",
                "自定义请求头 JSON 必须是字符串键值对象"
            )
        )
    })?;
    let mut headers = std::collections::HashMap::new();
    for (key, val) in obj {
        let text = match val {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        if !key.trim().is_empty() {
            headers.insert(key.clone(), text);
        }
    }
    Ok(headers)
}

fn normalize_extra_body(value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(String::new());
    }
    let parsed = serde_json::from_str::<Value>(value)?;
    if !parsed.is_object() {
        bail!(
            "{}",
            t(
                "Custom Body JSON must be a JSON object",
                "自定义 Body JSON 必须是 JSON 对象"
            )
        );
    }
    Ok(serde_json::to_string_pretty(&parsed)?)
}

/// 编辑模型配置表单。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `provider`: 当前 provider 配置
/// - `model`: 模型 ID
///
/// 返回:
/// - 是否保存
pub(super) fn edit_model_form(
    stdout: &mut io::Stdout,
    provider: &mut ProviderConfig,
    model: &str,
) -> Result<bool> {
    let original = provider.clone();
    let options = vec![
        t("General settings", "常规设置").to_string(),
        t("Model tags", "模型标签").to_string(),
        t("Save model settings", "保存模型设置").to_string(),
    ];
    let mut selected = 0usize;
    loop {
        draw_menu(
            stdout,
            &format!(" EDIT MODEL: {model} "),
            &options,
            selected,
            t("[Enter] open [q] cancel", "[Enter]打开 [q]取消"),
        )?;
        match read_key()? {
            crossterm::event::KeyCode::Up | crossterm::event::KeyCode::Char('k') => {
                selected = selected.saturating_sub(1)
            }
            crossterm::event::KeyCode::Down | crossterm::event::KeyCode::Char('j') => {
                selected = (selected + 1).min(options.len() - 1)
            }
            crossterm::event::KeyCode::Enter if selected == 0 => {
                edit_model_general_form(stdout, provider, model)?;
            }
            crossterm::event::KeyCode::Enter if selected == 1 => {
                edit_model_tags_form(stdout, provider, model)?;
            }
            crossterm::event::KeyCode::Enter => return Ok(true),
            crossterm::event::KeyCode::Esc | crossterm::event::KeyCode::Char('q') => {
                *provider = original;
                return Ok(false);
            }
            _ => {}
        }
    }
}

/// 编辑模型常规设置子面板。
fn edit_model_general_form(
    stdout: &mut io::Stdout,
    provider: &mut ProviderConfig,
    model: &str,
) -> Result<()> {
    let active = provider.models.iter().any(|item| item == model);
    let current = provider.default_model == model;
    let context_chars = context_chars_field_value(provider, model);
    let mut fields = vec![
        Field::boolean(t("Activate model", "激活模型"), active),
        Field::boolean(t("Set as current model", "设为当前模型"), current),
        tools_enabled_field(provider, model),
        Field::new(
            t("Model context tokens", "模型上下文 token 数"),
            context_chars,
        ),
        Field::new(
            t("Maximum output tokens", "最大输出 token 数"),
            max_output_tokens_field_value(provider, model),
        ),
        web_search_tool_mode_field(provider, model),
    ];
    if !run_form(stdout, t(" MODEL GENERAL ", " 模型常规设置 "), &mut fields)? {
        return Ok(());
    }
    let active = parse_bool_field(&fields[0].value)?;
    let current = parse_bool_field(&fields[1].value)?;
    if active {
        if !provider.models.iter().any(|item| item == model) {
            provider.models.push(model.to_string());
        }
    } else {
        provider.models.retain(|item| item != model);
    }
    if current || provider.default_model == model && !active {
        provider.default_model = if active {
            model.to_string()
        } else {
            provider.models.first().cloned().unwrap_or_default()
        };
        if !provider.default_model.is_empty()
            && !provider
                .models
                .iter()
                .any(|item| item == &provider.default_model)
        {
            provider.models.push(provider.default_model.clone());
        }
    }
    apply_tools_enabled_field(provider, model, &fields[2].value)?;
    apply_context_chars_field(provider, model, &fields[3].value)?;
    apply_max_output_tokens_field(provider, model, &fields[4].value)?;
    apply_web_search_tool_mode_field(provider, model, &fields[5].value);
    Ok(())
}

/// 编辑模型标签子面板。
fn edit_model_tags_form(
    stdout: &mut io::Stdout,
    provider: &mut ProviderConfig,
    model: &str,
) -> Result<()> {
    let mut fields = tag_fields(provider, model);
    if run_form(stdout, t(" MODEL TAGS ", " 模型标签 "), &mut fields)? {
        apply_tag_fields(provider, model, &fields)?;
    }
    Ok(())
}

/// 规范化 provider Base URL。
///
/// 参数:
/// - `value`: 表单输入值
///
/// 返回:
/// - 去除末尾斜杠和 chat completions 后缀后的 URL
fn normalize_base_url(value: &str) -> String {
    let mut url = value.trim().trim_end_matches('/').to_string();
    if url.ends_with("/chat/completions") {
        url.truncate(url.len() - "/chat/completions".len());
    }
    url
}

/// 把多密钥列表渲染成表单可编辑的多行文本。
///
/// 每行一个密钥，备注非空时以 ` | ` 分隔附在密钥之后。
///
/// 参数:
/// - `keys`: 多密钥列表
///
/// 返回:
/// - 多行文本；空列表返回空串
fn render_api_key_lines(keys: &[crate::config::ProviderApiKey]) -> String {
    keys.iter()
        .map(|key| {
            if key.label.is_empty() {
                key.api_key.clone()
            } else {
                format!("{} | {}", key.api_key, key.label)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 解析多行文本为多密钥列表，按密钥内容复用原标识以保持脱敏对齐。
///
/// 未在原列表出现的密钥分配新标识；空行被忽略。
///
/// 参数:
/// - `text`: 表单多行文本
/// - `original`: 原多密钥列表，用于按内容复用标识
///
/// 返回:
/// - 解析后的多密钥列表
fn parse_api_key_lines(
    text: &str,
    original: &[crate::config::ProviderApiKey],
) -> Vec<crate::config::ProviderApiKey> {
    let used: std::collections::HashSet<&str> =
        original.iter().map(|key| key.id.as_str()).collect();
    let mut next_id = 1usize;
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            // 1. 以首个 ` | ` 切分密钥与备注，密钥本身可能含竖线故只切第一段
            let (value, label) = match line.split_once(" | ") {
                Some((value, label)) => (value.trim().to_string(), label.trim().to_string()),
                None => (line.to_string(), String::new()),
            };
            // 2. 内容未变则复用原标识，保证与 Web 脱敏回填对齐
            let id = original
                .iter()
                .find(|key| key.api_key == value)
                .map(|key| key.id.clone())
                .unwrap_or_else(|| {
                    while used.contains(format!("key-{next_id}").as_str()) {
                        next_id += 1;
                    }
                    let fresh = format!("key-{next_id}");
                    next_id += 1;
                    fresh
                });
            crate::config::ProviderApiKey {
                id,
                api_key: value,
                label,
            }
        })
        .collect()
}

/// 把当前选中密钥渲染成 1 基序号文本。
///
/// 参数:
/// - `provider`: 当前供应商配置
///
/// 返回:
/// - 命中时返回序号字符串，否则空串
fn render_selected_index(provider: &ProviderConfig) -> String {
    let Some(selected) = provider.api_key_selected.as_deref() else {
        return String::new();
    };
    provider
        .api_keys
        .iter()
        .position(|key| key.id == selected)
        .map(|index| (index + 1).to_string())
        .unwrap_or_default()
}

/// 解析 1 基序号为对应的密钥标识。
///
/// 越界或非数字时返回空，表示回落到首个密钥。
///
/// 参数:
/// - `text`: 表单序号文本
/// - `keys`: 多密钥列表
///
/// 返回:
/// - 命中时返回密钥标识，否则 None
fn parse_selected_key(text: &str, keys: &[crate::config::ProviderApiKey]) -> Option<String> {
    let index: usize = text.trim().parse().ok()?;
    keys.get(index.checked_sub(1)?).map(|key| key.id.clone())
}
