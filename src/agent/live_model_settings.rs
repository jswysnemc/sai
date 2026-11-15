//! 模型设置在请求边界生效，正在传输的请求继续使用原客户端。

use super::Agent;
use crate::config::AppConfig;
use crate::llm::OpenAiCompatibleClient;
use crate::paths::SaiPaths;
use anyhow::Result;
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct LiveModelSettings {
    pending: Mutex<Option<(AppConfig, OpenAiCompatibleClient)>>,
}

impl LiveModelSettings {
    /// 【模型设置】【准备更新】参数：config 为新配置，paths 为存储路径；返回：验证结果，后一次更新覆盖前一次
    pub fn stage(&self, config: AppConfig, paths: &SaiPaths) -> Result<()> {
        let client = OpenAiCompatibleClient::from_config(&config, paths)?;
        config.active_context_window_tokens()?;
        config.active_model_tools_enabled()?;
        *self
            .pending
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some((config, client));
        Ok(())
    }
}

impl Agent {
    /// 【模型设置】【交互句柄】参数：当前会话；返回：与后台任务共用的设置入口
    pub fn live_model_settings(&self) -> Arc<LiveModelSettings> {
        self.live_model_settings.clone()
    }

    /// 【模型设置】【请求边界】参数：当前会话；返回：更新结果，只有请求间隙才能调用
    pub(super) fn apply_pending_model_settings(&mut self) -> Result<()> {
        let pending = self
            .live_model_settings
            .pending
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        if let Some((config, client)) = pending {
            // 1. 【模型设置】【会话重载】复用现有重载入口，保留会话、已加载工具和当前权限模式
            self.reload(config, client, self.tools.clone(), self.mode())?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "live_model_settings_tests.rs"]
mod tests;
