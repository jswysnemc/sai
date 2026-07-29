use super::{calculator, exchange_rate, hash_codec, moegirl, weather, xuanxue, ToolRegistry};
use crate::config::AppConfig;

/// 【CLI 助手工具】【运行时注册】按配置注册轻量内置可选工具。
///
/// 参数:
/// - `registry`: 待写入工具定义的注册表
/// - `config`: 当前应用配置
///
/// 返回:
/// - 无
pub(super) fn register(registry: &mut ToolRegistry, config: &AppConfig) {
    // 【CLI 助手工具】【可用性过滤】1. 每个工具只在对应开关启用时注册
    if config.plugins.weather.enabled {
        weather::register(registry);
    }
    if config.plugins.exchange_rate.enabled {
        exchange_rate::register(registry, config.plugins.exchange_rate.clone());
    }
    if config.plugins.xuanxue.enabled {
        xuanxue::register(registry);
    }
    if config.plugins.moegirl.enabled {
        moegirl::register(registry);
    }
    if config.plugins.hash_codec.enabled {
        hash_codec::register(registry);
    }
    if config.plugins.calculator.enabled {
        calculator::register(registry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIGURABLE_TOOL_NAMES: [&str; 7] = [
        "get_weather",
        "get_exchange_rate",
        "draw_zhouyi_hexagram",
        "query_moegirl",
        "calculate_hash",
        "decode_encoded_text",
        "scientific_calculator",
    ];

    /// 【CLI 助手工具】【运行时注册】验证关闭开关后不会向模型暴露工具。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn disabled_tools_are_not_registered() {
        let mut config = AppConfig::default();
        set_enabled(&mut config, false);
        let mut registry = ToolRegistry::new();

        register(&mut registry, &config);

        for name in CONFIGURABLE_TOOL_NAMES {
            assert!(
                !registry.contains(name),
                "unexpected registered tool: {name}"
            );
        }
    }

    /// 【CLI 助手工具】【运行时注册】验证启用开关后会向模型暴露工具。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn enabled_tools_are_registered() {
        let mut config = AppConfig::default();
        set_enabled(&mut config, true);
        let mut registry = ToolRegistry::new();

        register(&mut registry, &config);

        for name in CONFIGURABLE_TOOL_NAMES {
            assert!(registry.contains(name), "missing registered tool: {name}");
        }
    }

    /// 【CLI 助手工具】【默认可用性】验证默认配置沿用改造前的无条件注册行为。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn default_config_registers_all_configurable_tools() {
        let config = AppConfig::default();
        let mut registry = ToolRegistry::new();

        register(&mut registry, &config);

        for name in CONFIGURABLE_TOOL_NAMES {
            assert!(registry.contains(name), "missing default tool: {name}");
        }
    }

    /// 【CLI 助手工具】【测试配置】统一设置本组轻量工具的启用状态。
    ///
    /// 参数:
    /// - `config`: 待更新应用配置
    /// - `enabled`: 目标启用状态
    ///
    /// 返回:
    /// - 无
    fn set_enabled(config: &mut AppConfig, enabled: bool) {
        config.plugins.weather.enabled = enabled;
        config.plugins.exchange_rate.enabled = enabled;
        config.plugins.xuanxue.enabled = enabled;
        config.plugins.moegirl.enabled = enabled;
        config.plugins.hash_codec.enabled = enabled;
        config.plugins.calculator.enabled = enabled;
    }
}
