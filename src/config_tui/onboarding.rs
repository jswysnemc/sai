//! 首次进入聊天前配置供应商，复用现有终端菜单、表单和终端恢复逻辑。

use super::form::{run_form, Field};
use super::input::read_key;
use super::ui::{draw_menu_with_details, message};
use crate::config::onboarding::{complete_provider_setup, ProviderSetupInput};
use crate::config::{AppConfig, ProviderConfig};
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::Result;
use crossterm::event::KeyCode;
use std::io::{self, IsTerminal};

/// 【首次配置】【终端入口】尚未完成配置时打开引导，取消则退出本次聊天启动。
/// @param paths 为应用路径
/// @returns 可以继续进入聊天时为 true；用户取消时为 false
pub(crate) fn ensure_provider_setup(paths: &SaiPaths) -> Result<bool> {
    let config = AppConfig::load_or_default(paths)?;
    if config.provider_setup_complete || !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Ok(true);
    }
    super::session::run_provider_setup(paths, &config)
}

/// 【首次配置】【供应商菜单】选择模板后编辑连接字段，保存成功后结束引导。
/// @param stdout 为终端输出；paths 为应用路径；config 为当前配置
/// @returns 已完成时为 true；退出时为 false
pub(super) fn run(stdout: &mut io::Stdout, paths: &SaiPaths, config: &AppConfig) -> Result<bool> {
    let mut options: Vec<String> = config
        .providers
        .iter()
        .map(|provider| provider.display_name.clone())
        .collect();
    let mut details: Vec<String> = config.providers.iter().map(|provider| {
        if provider.is_opencode_zen() {
            t("Built-in free provider; no API key required. Confirm its model to start chatting.", "内置免费供应商，无需密钥。确认默认模型后开始聊天。").to_string()
        } else {
            format!("{}\n{}", provider.base_url, t("Enter an API key and default model. After saving, you can import the catalog in Settings.", "填写密钥和默认模型。保存后可在设置中导入模型目录。"))
        }
    }).collect();
    options.push(t("Custom provider", "自定义供应商").to_string());
    details.push(
        t(
            "Use your own compatible API address, key and model.",
            "填写自有兼容接口的地址、密钥和模型。",
        )
        .to_string(),
    );
    let mut selected = 0;
    loop {
        draw_menu_with_details(
            stdout,
            t(" FIRST USE · PROVIDER SETUP ", " 首次使用 · 配置供应商 "),
            &options,
            &details,
            selected,
            t(
                "Up/Down select · Enter configure · Esc exit",
                "上下选择 · Enter 配置 · Esc 退出",
            ),
            t(
                "Choose a provider before starting your first conversation",
                "开始首次对话前，请选择并配置供应商",
            ),
        )?;
        match read_key()? {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(false),
            KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => selected = (selected + 1).min(options.len() - 1),
            KeyCode::Enter => {
                let provider = config
                    .providers
                    .get(selected)
                    .cloned()
                    .unwrap_or_else(ProviderConfig::new_openai_compatible);
                if configure(stdout, paths, &provider, selected < config.providers.len())? {
                    return Ok(true);
                }
            }
            _ => {}
        }
    }
}

/// 【首次配置】【连接表单】收集接入信息，校验失败时保留输入并显示错误。
/// @param stdout 为终端输出；paths 为应用路径；provider 为模板；existing 表示复用已有供应商
/// @returns 保存成功时为 true；取消表单时为 false
fn configure(
    stdout: &mut io::Stdout,
    paths: &SaiPaths,
    provider: &ProviderConfig,
    existing: bool,
) -> Result<bool> {
    let mut fields = vec![
        Field::new(
            t("Provider name", "供应商名称"),
            provider.display_name.clone(),
        ),
        Field::new(t("API address", "API 地址"), provider.base_url.clone()),
        Field::new(t("Protocol", "协议"), provider.protocol.clone()).choices(&[
            "auto",
            "openai-chat",
            "openai-responses",
            "anthropic",
        ]),
        Field::new(
            t("API key or $env:NAME", "密钥或 $env:NAME"),
            provider.api_key.clone().unwrap_or_default(),
        )
        .secret(),
        Field::new(
            t("Default model", "默认模型"),
            provider.default_model.clone(),
        ),
    ];
    loop {
        if !run_form(
            stdout,
            t(
                " PROVIDER SETUP · SAVE TO START ",
                " 配置供应商 · 保存后开始使用 ",
            ),
            &mut fields,
        )? {
            return Ok(false);
        }
        let input = ProviderSetupInput {
            provider_id: existing.then(|| provider.id.clone()),
            display_name: fields[0].value.clone(),
            base_url: fields[1].value.clone(),
            protocol: fields[2].value.clone(),
            api_key: Some(fields[3].value.clone()),
            model: fields[4].value.clone(),
        };
        match complete_provider_setup(paths, input) {
            Ok(_) => return Ok(true),
            Err(error) => message(
                stdout,
                &format!(
                    "{}: {error}",
                    t("Could not save provider", "无法保存供应商")
                ),
            )?,
        }
    }
}
