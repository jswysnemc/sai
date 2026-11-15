//! 运行期间立即完成模型选择，配置重载与正在执行的请求分开处理。

use super::ReplRuntime;
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use anyhow::Result;

#[derive(Default)]
pub(in crate::cli) struct PendingSettings {
    pub(in crate::cli) reload: bool,
    pub(in crate::cli) clear_thinking_override: bool,
}

#[cfg(test)]
#[path = "stream_settings_tests.rs"]
mod tests;

impl ReplRuntime {
    /// 【终端设置】【任务绑定】参数：handle 为后台会话设置入口；返回：无
    pub(in crate::cli) fn bind_live_model_settings(
        &mut self,
        handle: std::sync::Arc<crate::agent::LiveModelSettings>,
    ) {
        self.live_model_settings = Some(handle);
    }

    /// 【终端设置】【待更新状态】参数：当前界面；返回：主循环是否需要重新加载设置
    pub(in crate::cli) fn has_pending_settings(&self) -> bool {
        self.pending_settings.reload
    }

    /// 【终端设置】【更新交接】参数：当前界面；返回：待更新标记，并清除已交接标记
    pub(in crate::cli) fn take_pending_settings(&mut self) -> PendingSettings {
        std::mem::take(&mut self.pending_settings)
    }

    /// 【终端设置】【设置保存】参数：paths 为路径，main_changed 为主模型变化；返回：准备更新结果
    fn stage_saved_settings(&mut self, paths: &SaiPaths, main_changed: bool) -> Result<()> {
        self.pending_settings.reload = true;
        self.pending_settings.clear_thinking_override |= main_changed;
        if main_changed {
            let config = crate::config::apply_agent_override(
                AppConfig::load(paths)?,
                None,
                crate::config::AgentSurface::Tui,
            )?;
            let provider = config.provider(None)?;
            let model = provider.default_model.clone();
            let thinking = provider.thinking_level.clone();
            let window = config.active_context_window_tokens()?;
            if let Some(handle) = &self.live_model_settings {
                handle.stage(config, paths)?;
            }
            // 【终端设置】【显示同步】选择完成后立即显示所选设置，当前传输仍使用原客户端
            if let Some(chrome) = self.last_chrome.as_mut() {
                chrome.set_model_settings(&model, &thinking, window);
                if let Some(composer) = self.composer.as_mut() {
                    composer.set_chrome(chrome.clone());
                }
            }
            self.redraw_stream_composer()?;
        }
        Ok(())
    }

    /// 【终端设置】【即时交互】参数：paths 为配置路径，input 为命令；返回：是否命中设置命令
    pub(super) fn run_stream_settings(&mut self, paths: &SaiPaths, input: &str) -> Result<bool> {
        use crate::cli::repl_commands::repl_command_rest;
        let model =
            repl_command_rest(input, "/model").or_else(|| repl_command_rest(input, "/providers"));
        let thinking =
            repl_command_rest(input, "/thinking").or_else(|| repl_command_rest(input, "/think"));
        if model.is_none() && thinking.is_none() {
            return Ok(false);
        }
        // 1. 【终端设置】【交互执行】后台任务持有 Agent，选择器只操作配置与终端输入
        let result = (|| -> Result<Option<(String, bool)>> {
            if let Some(rest) = model {
                if rest.is_empty() {
                    return Ok(match crate::cli::models_picker::run_interactive(paths)? {
                        crate::cli::models_picker::PickerOutcome::Cancelled => None,
                        crate::cli::models_picker::PickerOutcome::Saved {
                            message,
                            main_model_changed,
                        } => Some((message, main_model_changed)),
                    });
                }
                let selection = rest.parse::<usize>().map_err(|_| {
                    anyhow::anyhow!(crate::i18n::text(
                        "usage: /model [index]",
                        "用法: /model [序号]"
                    ))
                })?;
                let result = crate::control_commands::run_model_command(
                    paths,
                    Some(selection),
                    crate::control_commands::ControlSurface::Repl,
                )?;
                return Ok(Some((result.message, result.changed)));
            }
            let rest = thinking.unwrap_or_default();
            if rest.split_whitespace().count() > 1 {
                anyhow::bail!("usage: /think [level]");
            }
            crate::cli::providers::run_set_thinking(
                paths,
                crate::cli::SetThinkingArgs {
                    level: (!rest.is_empty()).then(|| rest.to_string()),
                },
            )?;
            Ok(Some((
                crate::i18n::text("thinking settings saved", "思考设置已保存").into(),
                true,
            )))
        })();
        // 2. 【终端设置】【画面恢复】选择器关闭后重绘，保留正文位置与未提交草稿
        self.redraw()?;
        match result {
            Ok(Some((message, main_changed))) => {
                if let Err(error) = self.stage_saved_settings(paths, main_changed) {
                    self.record_meta(error.to_string())?;
                } else {
                    self.record_meta(format!(
                        "{message}\n{}",
                        crate::i18n::text(
                            "The next model request will use these settings",
                            "下一次模型请求将使用新设置",
                        )
                    ))?;
                }
            }
            Ok(None) => self.record_meta(
                crate::i18n::text("model selection cancelled", "已取消模型选择").into(),
            )?,
            Err(error) => self.record_meta(error.to_string())?,
        }
        Ok(true)
    }
}
