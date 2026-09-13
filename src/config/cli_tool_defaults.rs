use super::cli_tools::*;
use super::defaults::*;
use super::model::MemoryConfig;

impl Default for PluginsConfig {
    /// 构造全部 CLI 助手工具与 Web 搜索的默认配置。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 历史 `plugins` 键对应的完整默认配置
    fn default() -> Self {
        Self {
            vision: VisionPluginConfig::default(),
            calculator: CalculatorPluginConfig::default(),
            memory: MemoryConfig::default(),
        }
    }
}

impl Default for VisionPluginConfig {
    /// 构造视觉理解工具默认配置。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 当前模型优先的默认视觉配置
    fn default() -> Self {
        Self {
            enabled: default_true(),
            prefer_current_multimodal_model: default_true(),
            vision_provider_id: String::new(),
            vision_model: String::new(),
            preview_with_chafa: default_true(),
        }
    }
}

impl Default for CalculatorPluginConfig {
    /// 构造计算器工具默认配置。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 默认启用且使用内置后端的配置
    fn default() -> Self {
        Self {
            enabled: default_true(),
            backend: default_calculator_backend(),
        }
    }
}
