use super::*;
use crate::config::AppConfig;
use crate::control_commands::agent::agent_choices;

/// 交互式模糊选择 Agent，返回 1 基序号。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - 选中的 1 基序号；取消时返回空
pub(super) fn select_agent_index_interactively(paths: &SaiPaths) -> Result<Option<usize>> {
    AppConfig::init_files(paths)?;
    let config = AppConfig::load_or_default(paths)?;
    let choices = agent_choices(&config);
    if choices.is_empty() {
        bail!("{}", t("no agents available", "没有可用的 Agent"));
    }
    let active = config
        .tui_agent
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(crate::config::DEFAULT_AGENT_ID);
    let labels = choices
        .iter()
        .enumerate()
        .map(|(index, choice)| {
            let marker = if choice.id == active { "*" } else { " " };
            let desc = if choice.description.trim().is_empty() {
                String::new()
            } else {
                format!(" — {}", choice.description.trim())
            };
            format!(
                "{marker} {}. {} ({}){desc}",
                index + 1,
                choice.name,
                choice.id
            )
        })
        .collect::<Vec<_>>();
    let Some(index) = inline_fuzzy_select(&labels)? else {
        return Ok(None);
    };
    Ok(Some(index + 1))
}
