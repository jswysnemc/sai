use super::model::ProviderConfig;
use super::model_units::deserialize_optional_context_chars;
use serde::{Deserialize, Serialize};

pub const MODEL_TAG_TOOL: &str = "tool";
pub const MODEL_TAG_THINKING: &str = "thinking";
pub const MODEL_TAG_VISION: &str = "vision";
pub const MODEL_TAG_WEB_SEARCH: &str = "web_search";
pub const MODEL_TAG_FAST: &str = "fast";
pub const MODEL_TAG_LOW_COST: &str = "low_cost";
pub const WEB_SEARCH_TOOL_MODE_ENABLED: &str = "enabled";
pub const WEB_SEARCH_TOOL_MODE_HIDE: &str = "hide_builtin";
pub const WEB_SEARCH_TOOL_MODE_RENAME: &str = "rename_local";
pub const DEEPSEEK_ANCHOR_MODE_OFF: &str = "off";
pub const DEEPSEEK_ANCHOR_MODE_STANDARD: &str = "anchored_standard";
pub const DEEPSEEK_ANCHOR_MODES: [&str; 2] =
    [DEEPSEEK_ANCHOR_MODE_OFF, DEEPSEEK_ANCHOR_MODE_STANDARD];
pub const MODEL_TAGS: [&str; 6] = [
    MODEL_TAG_TOOL,
    MODEL_TAG_THINKING,
    MODEL_TAG_VISION,
    MODEL_TAG_WEB_SEARCH,
    MODEL_TAG_FAST,
    MODEL_TAG_LOW_COST,
];

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelMetadata {
    #[serde(
        default,
        deserialize_with = "deserialize_optional_context_chars",
        skip_serializing_if = "Option::is_none"
    )]
    pub context_chars: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools_enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// 该模型实际支持的思考等级；空表示未知，界面展示全部。
    ///
    /// 目录数据可能滞后或有误，因此这里是可编辑的覆盖点而不是只读探测结果：
    /// 清空即恢复展示全部等级。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub thinking_levels: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_search_tool_mode: Option<String>,
    /// DeepSeek 类模型的首请求工具锚定模式；缺失表示关闭。
    #[serde(default, skip_serializing_if = "is_none_or_off")]
    pub deepseek_anchor_mode: Option<String>,
}

fn is_none_or_off(value: &Option<String>) -> bool {
    value
        .as_deref()
        .map(|mode| mode == DEEPSEEK_ANCHOR_MODE_OFF)
        .unwrap_or(true)
}

impl ModelMetadata {
    /// 判断模型元数据是否为空。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 没有上下文长度、工具开关和标签时返回 true
    pub fn is_empty(&self) -> bool {
        self.context_chars.is_none()
            && self.max_output_tokens.is_none()
            && self.tools_enabled.is_none()
            && self.tags.is_empty()
            && self.thinking_levels.is_empty()
            && self.web_search_tool_mode.is_none()
            && is_none_or_off(&self.deepseek_anchor_mode)
    }
}

impl ProviderConfig {
    /// 获取模型上下文 token 数。
    ///
    /// 参数:
    /// - `model`: 模型 ID
    ///
    /// 返回:
    /// - 新模型元数据中的上下文 token 数，或旧字段中的兼容值
    pub fn model_context_chars_for(&self, model: &str) -> Option<usize> {
        self.model_metadata
            .get(model)
            .and_then(|metadata| metadata.context_chars)
            .or_else(|| self.model_context_chars.get(model).copied())
    }

    /// 获取模型标签。
    ///
    /// 参数:
    /// - `model`: 模型 ID
    ///
    /// 返回:
    /// - 模型标签列表
    pub fn model_tags_for(&self, model: &str) -> &[String] {
        self.model_metadata
            .get(model)
            .map(|metadata| metadata.tags.as_slice())
            .unwrap_or(&[])
    }

    /// 获取模型支持的思考等级。
    ///
    /// 参数:
    /// - `model`: 模型 ID
    ///
    /// 返回:
    /// - 支持的等级列表；空表示未知，调用方应视为全部可用
    pub fn model_thinking_levels_for(&self, model: &str) -> &[String] {
        self.model_metadata
            .get(model)
            .map(|metadata| metadata.thinking_levels.as_slice())
            .unwrap_or(&[])
    }

    /// 设置模型支持的思考等级。
    ///
    /// 参数:
    /// - `model`: 模型 ID
    /// - `levels`: 已校验的等级列表；传空表示恢复为未知
    ///
    /// 返回:
    /// - 无
    pub fn set_model_thinking_levels_for(&mut self, model: &str, levels: Vec<String>) {
        if model.trim().is_empty() {
            return;
        }
        if levels.is_empty() {
            if let Some(metadata) = self.model_metadata.get_mut(model) {
                metadata.thinking_levels.clear();
            }
        } else {
            self.model_metadata_mut(model).thinking_levels = levels;
        }
        self.remove_empty_model_metadata(model);
    }

    /// 判断某个思考等级对指定模型是否可用。
    ///
    /// 参数:
    /// - `model`: 模型 ID
    /// - `level`: 待判定的思考等级
    ///
    /// 返回:
    /// - 未记录支持范围或等级在范围内时为真
    pub fn model_supports_thinking_level(&self, model: &str, level: &str) -> bool {
        super::model_thinking::thinking_level_available(
            self.model_thinking_levels_for(model),
            level,
        )
    }

    /// 判断模型是否允许工具调用。
    ///
    /// 参数:
    /// - `model`: 模型 ID
    ///
    /// 返回:
    /// - 没有显式关闭时返回 true
    pub fn model_tools_enabled_for(&self, model: &str) -> bool {
        self.model_metadata
            .get(model)
            .and_then(|metadata| metadata.tools_enabled)
            .unwrap_or(true)
    }

    /// 返回模型的网页搜索工具冲突策略。
    ///
    /// 参数:
    /// - `model`: 模型 ID
    ///
    /// 返回:
    /// - 未配置时返回启用本地 Web Search
    pub fn model_web_search_tool_mode_for(&self, model: &str) -> &str {
        self.model_metadata
            .get(model)
            .and_then(|metadata| metadata.web_search_tool_mode.as_deref())
            .unwrap_or(WEB_SEARCH_TOOL_MODE_ENABLED)
    }

    /// 设置模型的网页搜索工具冲突策略。
    ///
    /// 参数:
    /// - `model`: 模型 ID
    /// - `mode`: 隐藏内置冲突工具或更名本地工具
    ///
    /// 返回:
    /// - 无
    pub fn set_model_web_search_tool_mode(&mut self, model: &str, mode: Option<String>) {
        if model.trim().is_empty() {
            return;
        }
        self.model_metadata_mut(model).web_search_tool_mode = mode;
        self.remove_empty_model_metadata(model);
    }

    /// 返回模型的 DeepSeek 首请求锚定模式。
    pub fn model_deepseek_anchor_mode_for(&self, model: &str) -> &str {
        self.model_metadata
            .get(model)
            .and_then(|metadata| metadata.deepseek_anchor_mode.as_deref())
            .unwrap_or(DEEPSEEK_ANCHOR_MODE_OFF)
    }

    /// 设置模型的 DeepSeek 首请求锚定模式。
    pub fn set_model_deepseek_anchor_mode_for(&mut self, model: &str, mode: &str) {
        if model.trim().is_empty() {
            return;
        }
        let value = mode.trim();
        if value == DEEPSEEK_ANCHOR_MODE_OFF || value.is_empty() {
            if let Some(metadata) = self.model_metadata.get_mut(model) {
                metadata.deepseek_anchor_mode = None;
            }
        } else {
            self.model_metadata_mut(model).deepseek_anchor_mode = Some(value.to_string());
        }
        self.remove_empty_model_metadata(model);
    }

    /// 设置模型上下文 token 数。
    ///
    /// 参数:
    /// - `model`: 模型 ID
    /// - `context_chars`: 上下文 token 数，空值表示取消配置
    ///
    /// 返回:
    /// - 无
    pub fn set_model_context_chars_for(&mut self, model: &str, context_chars: Option<usize>) {
        if model.trim().is_empty() {
            return;
        }
        self.model_context_chars.remove(model);
        if let Some(context_chars) = context_chars {
            self.model_metadata_mut(model).context_chars = Some(context_chars);
        } else if let Some(metadata) = self.model_metadata.get_mut(model) {
            metadata.context_chars = None;
        }
        self.remove_empty_model_metadata(model);
    }

    /// 获取模型最大输出 token 数。
    ///
    /// 参数:
    /// - `model`: 模型 ID
    ///
    /// 返回:
    /// - 模型级最大输出限制
    pub fn model_max_output_tokens_for(&self, model: &str) -> Option<u32> {
        self.model_metadata
            .get(model)
            .and_then(|metadata| metadata.max_output_tokens)
    }

    /// 设置模型最大输出 token 数。
    ///
    /// 参数:
    /// - `model`: 模型 ID
    /// - `max_output_tokens`: 最大输出 token 数，空值表示不限制
    pub fn set_model_max_output_tokens_for(&mut self, model: &str, max_output_tokens: Option<u32>) {
        if model.trim().is_empty() {
            return;
        }
        if let Some(value) = max_output_tokens {
            self.model_metadata_mut(model).max_output_tokens = Some(value);
        } else if let Some(metadata) = self.model_metadata.get_mut(model) {
            metadata.max_output_tokens = None;
        }
        self.remove_empty_model_metadata(model);
    }

    /// 设置模型标签。
    ///
    /// 参数:
    /// - `model`: 模型 ID
    /// - `tags`: 已校验的标签列表
    ///
    /// 返回:
    /// - 无
    pub fn set_model_tags_for(&mut self, model: &str, tags: Vec<String>) {
        if model.trim().is_empty() {
            return;
        }
        if tags.is_empty() {
            if let Some(metadata) = self.model_metadata.get_mut(model) {
                metadata.tags.clear();
            }
        } else {
            self.model_metadata_mut(model).tags = tags;
        }
        self.remove_empty_model_metadata(model);
    }

    /// 设置模型工具调用开关。
    ///
    /// 参数:
    /// - `model`: 模型 ID
    /// - `enabled`: 是否允许工具调用，true 会恢复默认兼容行为
    ///
    /// 返回:
    /// - 无
    pub fn set_model_tools_enabled_for(&mut self, model: &str, enabled: bool) {
        if model.trim().is_empty() {
            return;
        }
        if enabled {
            if let Some(metadata) = self.model_metadata.get_mut(model) {
                metadata.tools_enabled = None;
            }
        } else {
            self.model_metadata_mut(model).tools_enabled = Some(false);
        }
        self.remove_empty_model_metadata(model);
    }

    /// 获取可修改的模型元数据。
    ///
    /// 参数:
    /// - `model`: 模型 ID
    ///
    /// 返回:
    /// - 模型元数据引用
    fn model_metadata_mut(&mut self, model: &str) -> &mut ModelMetadata {
        self.model_metadata.entry(model.to_string()).or_default()
    }

    /// 清理空模型元数据。
    ///
    /// 参数:
    /// - `model`: 模型 ID
    ///
    /// 返回:
    /// - 无
    fn remove_empty_model_metadata(&mut self, model: &str) {
        if self
            .model_metadata
            .get(model)
            .map(ModelMetadata::is_empty)
            .unwrap_or(false)
        {
            self.model_metadata.remove(model);
        }
    }
}

/// 判断模型标签是否合法。
///
/// 参数:
/// - `tag`: 标签名称
///
/// 返回:
/// - 标签属于内置标签集合时返回 true
pub fn is_valid_model_tag(tag: &str) -> bool {
    MODEL_TAGS.contains(&tag.trim())
}

/// 判断供应商与模型标识是否属于 DeepSeek 类模型。
pub fn is_deepseek_like_model(provider: &ProviderConfig, model: &str) -> bool {
    provider.id.to_ascii_lowercase().contains("deepseek")
        || provider.base_url.to_ascii_lowercase().contains("deepseek")
        || model.to_ascii_lowercase().contains("deepseek")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deepseek_anchor_mode_is_model_scoped_and_default_is_removed() {
        let mut provider = ProviderConfig::new_openai_compatible();

        provider.set_model_deepseek_anchor_mode_for("deepseek-v4", DEEPSEEK_ANCHOR_MODE_STANDARD);

        assert_eq!(
            provider.model_deepseek_anchor_mode_for("deepseek-v4"),
            DEEPSEEK_ANCHOR_MODE_STANDARD
        );
        assert_eq!(
            provider.model_deepseek_anchor_mode_for("other-model"),
            DEEPSEEK_ANCHOR_MODE_OFF
        );

        provider.set_model_deepseek_anchor_mode_for("deepseek-v4", DEEPSEEK_ANCHOR_MODE_OFF);
        assert!(!provider.model_metadata.contains_key("deepseek-v4"));
    }

    #[test]
    fn detects_deepseek_from_provider_or_model_identity() {
        let mut provider = ProviderConfig::new_openai_compatible();
        provider.id = "gateway".to_string();
        provider.base_url = "https://example.test/v1".to_string();

        assert!(is_deepseek_like_model(&provider, "deepseek-v4"));
        assert!(!is_deepseek_like_model(&provider, "other-model"));
    }
}
