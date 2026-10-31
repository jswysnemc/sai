use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CodeMode {
    #[default]
    On,
    Only,
    Off,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CodeModeConfig {
    pub mode: CodeMode,
    pub inline_budget: usize,
    pub timeout_ms: u64,
}

impl Default for CodeModeConfig {
    /// 【CodeMode】【默认配置】无参数，返回保留直接工具声明的默认配置。
    fn default() -> Self {
        Self {
            mode: CodeMode::On,
            inline_budget: 3000,
            timeout_ms: 60_000,
        }
    }
}

impl CodeModeConfig {
    /// 【CodeMode】【配置校验】校验自身预算与超时范围，返回校验结果。
    pub fn validate(&self) -> Result<()> {
        if self.timeout_ms == 0 || self.timeout_ms > 600_000 {
            bail!("codemode.timeout_ms must be between 1 and 600000");
        }
        if self.inline_budget > 32_000 {
            bail!("codemode.inline_budget must not exceed 32000");
        }
        Ok(())
    }
}
