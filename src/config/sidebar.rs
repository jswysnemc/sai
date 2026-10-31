use serde::{Deserialize, Serialize};

/// 侧栏启动偏好；手动开启只覆盖当前会话，不写入默认配置。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SidebarDefault {
    /// 空间足够时显示，窄屏或矮屏隐藏
    #[default]
    Auto,
    /// 启动时隐藏，仍允许使用命令开启
    Off,
}

#[cfg(test)]
mod tests {
    use super::SidebarDefault;
    use crate::config::DisplayConfig;

    /// 【显示配置】【侧栏默认值】验证旧配置兼容、序列化和非法值拒绝。
    /// 参数: 无；返回: 无。
    #[test]
    fn sidebar_defaults_and_roundtrips() {
        let legacy: DisplayConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(legacy.sidebar, SidebarDefault::Auto);
        for (name, expected) in [("auto", SidebarDefault::Auto), ("off", SidebarDefault::Off)] {
            let value = serde_json::json!({"sidebar": name});
            let config: DisplayConfig = serde_json::from_value(value).unwrap();
            assert_eq!(config.sidebar, expected);
            assert_eq!(serde_json::to_value(config).unwrap()["sidebar"], name);
        }
        assert!(serde_json::from_str::<DisplayConfig>(r#"{"sidebar":"on"}"#).is_err());
        assert_eq!(DisplayConfig::default().sidebar, SidebarDefault::Auto);
    }
}
