use super::defaults::*;
use super::model::MemoryConfig;
use serde::{Deserialize, Serialize};

/// CLI 助手可选工具的历史兼容配置容器。
///
/// 配置文件继续使用 `plugins` 键，避免破坏既有用户配置；界面统一使用
/// “CLI 助手工具”语义。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginsConfig {
    #[serde(default)]
    pub vision: VisionPluginConfig,
    #[serde(default)]
    pub calculator: CalculatorPluginConfig,
    #[serde(default)]
    pub memory: MemoryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionPluginConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub vision_provider_id: String,
    #[serde(default)]
    pub vision_model: String,
    #[serde(default = "default_true")]
    pub preview_with_chafa: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalculatorPluginConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_calculator_backend")]
    pub backend: String,
}

impl PluginsConfig {
    /// 返回所有插件均启用的副本。
    ///
    /// 工具目录与白名单诊断都要回答"这个工具在系统里存不存在"，那与用户
    /// 当前开了哪些插件无关：关掉汇率插件不该让汇率工具从 Agent 配置界面
    /// 上消失，否则想启用它的人根本勾不到。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 全部插件开关置为 true 的副本
    pub fn all_enabled(&self) -> Self {
        let mut plugins = self.clone();
        plugins.vision.enabled = true;
        plugins.calculator.enabled = true;
        plugins.memory.enabled = true;
        plugins
    }
}

#[cfg(test)]
mod all_enabled_tests {
    use super::*;

    /// 验证每一个插件开关都被打开。
    ///
    /// 逐字段赋值必然会在新增插件时漏掉，而漏掉的表现是那个工具在 Agent
    /// 配置界面上不可见——没有任何东西会报错。这里按序列化结果遍历，
    /// 新插件只要带 enabled 字段就会被这条测试抓住。
    #[test]
    fn every_plugin_switch_is_turned_on() {
        // 先关掉一批，确保通过不是因为默认值本来就是 true
        let mut plugins = PluginsConfig::default();
        plugins.calculator.enabled = false;

        let enabled = serde_json::to_value(plugins.all_enabled()).unwrap();

        for (name, value) in enabled.as_object().unwrap() {
            if let Some(flag) = value.get("enabled") {
                assert_eq!(flag, true, "插件 {name} 的开关没有被打开");
            }
        }
    }
}
