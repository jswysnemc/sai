use super::*;
use crate::config::AppConfig;

/// 交互式模糊选择模型，返回 1 基序号。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - 选中的 1 基模型序号；取消时返回空
pub(super) fn select_model_index_interactively(paths: &SaiPaths) -> Result<Option<usize>> {
    AppConfig::init_files(paths)?;
    let config = AppConfig::load_or_default(paths)?;
    let choices = config.provider_model_choices();
    if choices.is_empty() {
        bail!(
            "{}",
            t(
                "no active provider models; configure or activate a model first",
                "没有已激活的 provider 模型；请先配置或激活模型",
            )
        );
    }
    let active = config.provider(None).ok();
    let labels = choices
        .iter()
        .enumerate()
        .map(|(index, choice)| {
            let marker = if active
                .map(|provider| {
                    provider.id == choice.provider_id && provider.default_model == choice.model
                })
                .unwrap_or(false)
            {
                "*"
            } else {
                " "
            };
            format!("{marker} {}. {}", index + 1, choice.label())
        })
        .collect::<Vec<_>>();
    let Some(index) = inline_fuzzy_select(&labels)? else {
        return Ok(None);
    };
    Ok(Some(index + 1))
}
