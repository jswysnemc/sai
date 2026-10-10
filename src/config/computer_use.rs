//! 原生桌面操作的显式开关与本地驱动配置，不自动安装或申请系统权限。

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

/// Computer Use 配置；旧配置默认关闭，避免升级后扩大桌面访问权限。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ComputerUseConfig {
    /// 用户是否明确启用原生桌面操作
    pub enabled: bool,
    /// 本地 cua-driver 可执行文件，可使用绝对路径
    pub driver_path: String,
    /// 单次原生调用的超时上限，毫秒
    pub timeout_ms: u64,
}

impl Default for ComputerUseConfig {
    /// 【桌面操作】【默认配置】保持功能关闭，仅声明驱动名称和有界超时。
    /// @returns 无需启动原生进程的默认配置
    fn default() -> Self {
        Self {
            enabled: false,
            driver_path: "cua-driver".into(),
            timeout_ms: 30_000,
        }
    }
}

impl ComputerUseConfig {
    /// 【桌面操作】【配置校验】拒绝空驱动路径、空字符和无界超时。
    /// @returns 配置合法时成功，否则返回字段级错误
    pub(crate) fn validate(&self) -> Result<()> {
        if self.driver_path.trim().is_empty() || self.driver_path.contains('\0') {
            bail!("computer_use.driver_path must name a local cua-driver executable");
        }
        if !(1_000..=120_000).contains(&self.timeout_ms) {
            bail!("computer_use.timeout_ms must be between 1000 and 120000");
        }
        Ok(())
    }
}

/// 【桌面操作】【配置兼容】省略未启用的默认配置，保持旧配置文件简洁。
/// @param config 为待序列化配置
/// @returns 是否等于默认配置
pub(super) fn is_default(config: &ComputerUseConfig) -> bool {
    config == &ComputerUseConfig::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【桌面测试】【旧配置】没有桌面配置的旧文件保持关闭且可无损往返。
    #[test]
    fn old_config_does_not_enable_native_access() {
        let old = serde_json::json!({"active_provider":"fixture", "providers":[]});
        let config: crate::config::AppConfig = serde_json::from_value(old).unwrap();
        assert!(!config.computer_use.enabled);
        assert_eq!(config.computer_use.driver_path, "cua-driver");
        assert!(serde_json::to_value(config)
            .unwrap()
            .get("computer_use")
            .is_none());
    }

    /// 【桌面测试】【显式配置】启用状态、带空格的路径和超时能够往返保存。
    #[test]
    fn explicit_configuration_round_trips() {
        let value = serde_json::json!({
            "enabled":true, "driver_path":"/opt/Native Driver/cua-driver", "timeout_ms":5000
        });
        let config: ComputerUseConfig = serde_json::from_value(value.clone()).unwrap();
        config.validate().unwrap();
        assert_eq!(serde_json::to_value(config).unwrap(), value);
        for timeout_ms in [0, 999, 120_001, u64::MAX] {
            assert!(ComputerUseConfig {
                timeout_ms,
                ..Default::default()
            }
            .validate()
            .is_err());
        }
    }
}
