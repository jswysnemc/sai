use super::{AppConfig, ProviderConfig, ProviderModelChoice};
use anyhow::{bail, Context, Result};

impl AppConfig {
    /// 汇总一个供应商可选择的模型标识。
    ///
    /// `default_model` 可能不在 `models` 列表里：`/config` 里的几处编辑路径
    /// 只改写 `default_model` 而不追加进 `models`，直接手改配置文件同理。
    /// 只读 `models` 会让当前生效的模型在 `/model` 里凭空消失，所以补上。
    ///
    /// 参数:
    /// - `provider`: 供应商配置
    ///
    /// 返回:
    /// - 去重且去空的模型标识列表
    fn provider_choice_models(provider: &ProviderConfig) -> Vec<String> {
        let mut models = provider.models.clone();
        let default_model = provider.default_model.trim();
        if !default_model.is_empty() && !models.iter().any(|model| model == default_model) {
            models.push(default_model.to_string());
        }
        let mut seen = std::collections::HashSet::new();
        models
            .into_iter()
            .filter_map(|model| {
                let model = model.trim();
                (!model.is_empty() && seen.insert(model.to_string())).then(|| model.to_string())
            })
            .collect()
    }

    /// 枚举可供选择的供应商模型组合。
    ///
    /// 已停用的供应商整体跳过，其模型不出现在任何选择列表里。
    ///
    /// 返回:
    /// - 供应商与模型的组合列表
    pub fn provider_model_choices(&self) -> Vec<ProviderModelChoice> {
        self.providers
            .iter()
            .filter(|provider| provider.enabled)
            .flat_map(|provider| {
                Self::provider_choice_models(provider)
                    .into_iter()
                    .map(|model| ProviderModelChoice {
                        provider_id: provider.id.clone(),
                        provider_name: provider.display_name.clone(),
                        model,
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// 设置当前模型；参数为已启用供应商标识及模型名称，返回校验与更新结果。
    pub fn set_active_provider_model(&mut self, provider_id: &str, model: &str) -> Result<()> {
        let provider = self
            .providers
            .iter_mut()
            .find(|provider| provider.id == provider_id)
            .with_context(|| format!("provider not found: {provider_id}"))?;
        if !provider.enabled {
            bail!("provider is disabled: {provider_id}");
        }
        if model.trim().is_empty() {
            bail!("model cannot be empty");
        }
        self.active_provider = provider.id.clone();
        provider.default_model = model.to_string();
        if !provider.models.iter().any(|item| item == model) {
            provider.models.push(model.to_string());
        }
        Ok(())
    }

    /// 按模型标签选择当前 provider 和模型。
    ///
    /// 参数:
    /// - `tag`: 模型标签
    ///
    /// 返回:
    /// - 被选中的 provider/model
    pub fn select_active_provider_model_with_tag(
        &mut self,
        tag: &str,
    ) -> Result<ProviderModelChoice> {
        let tag = tag.trim();
        if tag.is_empty() {
            bail!("model tag cannot be empty");
        }
        let choices = self.provider_model_choices_with_tag(tag);
        let choice = choices
            .iter()
            .find(|choice| {
                self.active_provider == choice.provider_id
                    && self
                        .provider(Some(&choice.provider_id))
                        .map(|provider| provider.default_model == choice.model)
                        .unwrap_or(false)
            })
            .or_else(|| choices.first())
            .cloned()
            .with_context(|| format!("no active provider model has tag: {tag}"))?;
        self.set_active_provider_model(&choice.provider_id, &choice.model)?;
        Ok(choice)
    }

    /// 返回拥有指定标签的 provider/model 列表。
    ///
    /// 已停用的供应商整体跳过，与不带标签的枚举保持一致。
    ///
    /// 参数:
    /// - `tag`: 模型标签
    ///
    /// 返回:
    /// - 匹配的 provider/model 列表
    pub fn provider_model_choices_with_tag(&self, tag: &str) -> Vec<ProviderModelChoice> {
        let tag = tag.trim();
        self.providers
            .iter()
            .filter(|provider| provider.enabled)
            .flat_map(|provider| {
                Self::provider_choice_models(provider)
                    .into_iter()
                    .filter(|model| {
                        !model.trim().is_empty()
                            && provider
                                .model_tags_for(model)
                                .iter()
                                .any(|item| item == tag)
                    })
                    .map(|model| ProviderModelChoice {
                        provider_id: provider.id.clone(),
                        provider_name: provider.display_name.clone(),
                        model,
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }
}
