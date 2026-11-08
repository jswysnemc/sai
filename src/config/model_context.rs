use super::app_validation::default_context_chars_for_provider_model;
use super::ProviderConfig;

impl ProviderConfig {
    /// 【上下文】【模型上限】读取用户选择的使用上限。
    /// 参数: `model` 为模型标识；返回: token 上限，空表示使用模型最大容量
    pub fn model_context_limit_tokens_for(&self, model: &str) -> Option<usize> {
        self.model_metadata
            .get(model)
            .and_then(|metadata| metadata.context_limit_tokens)
    }

    /// 【上下文】【模型上限】保存使用上限，保留 API 容量及其他元数据。
    /// 参数: `model` 为模型标识，`limit` 为正 token 数或空；返回: 无
    pub fn set_model_context_limit_tokens_for(&mut self, model: &str, limit: Option<usize>) {
        if model.trim().is_empty() {
            return;
        }
        let limit = limit.filter(|value| *value > 0);
        if let Some(limit) = limit {
            self.model_metadata
                .entry(model.to_string())
                .or_default()
                .context_limit_tokens = Some(limit);
        } else if let Some(metadata) = self.model_metadata.get_mut(model) {
            metadata.context_limit_tokens = None;
            if metadata.is_empty() {
                self.model_metadata.remove(model);
            }
        }
    }

    /// 【上下文】【模型容量】读取目录或内置规则提供的容量，不包含用户上限。
    /// 参数: `model` 为模型标识；返回: 已知最大 token 数，未知时为空
    pub fn model_context_capacity_tokens_for(&self, model: &str) -> Option<usize> {
        self.model_context_chars_for(model)
            .or_else(|| default_context_chars_for_provider_model(self, model))
    }

    /// 【上下文】【预算计算】将用户上限约束到已知容量，容量未知时允许手动指定。
    /// 参数: `model` 为模型标识，`fallback` 为容量未知时的默认值；返回: 实际预算 token 数
    pub fn effective_context_tokens_for(&self, model: &str, fallback: usize) -> usize {
        match (
            self.model_context_limit_tokens_for(model),
            self.model_context_capacity_tokens_for(model),
        ) {
            (Some(limit), Some(capacity)) => limit.min(capacity),
            (Some(limit), None) => limit,
            (None, Some(capacity)) => capacity,
            (None, None) => fallback,
        }
    }
}
