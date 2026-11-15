//! 输入框样式偏好和命令，独立于模型配置与会话正文。

use super::ReplRuntime;
use crate::config::{AppConfig, InputStyle};
use anyhow::Result;

#[derive(Default)]
pub(super) struct InputStyleState {
    pub(super) default: InputStyle,
    pub(super) selected: Option<InputStyle>,
}

impl InputStyleState {
    /// 【输入样式】【有效偏好】参数：当前状态；返回：手动选择或配置默认值
    pub(super) fn current(&self) -> InputStyle {
        self.selected.unwrap_or(self.default)
    }
}

impl ReplRuntime {
    /// 【输入样式】【配置同步】参数：style 为配置样式；返回：无，保留本终端手动选择
    pub(in crate::cli) fn configure_input_style(&mut self, style: InputStyle) {
        self.input_style.default = style;
        if let Some(composer) = self.composer.as_mut() {
            composer.set_input_style(self.input_style.current());
        }
    }

    /// 【输入样式】【命令处理】参数：args 为 /tui 后的参数；返回：是否命中输入样式命令
    pub(super) fn handle_input_style_command(&mut self, args: &[String]) -> Result<bool> {
        if args.first().map(String::as_str) != Some("input") {
            return Ok(false);
        }
        if args.len() == 1 {
            self.record_meta(format!("input: {}", self.input_style.current().as_str()))?;
            return Ok(true);
        }
        let style = (args.len() == 2)
            .then(|| InputStyle::parse(&args[1]))
            .flatten();
        let Some(style) = style else {
            self.record_meta(
                crate::i18n::text(
                    "usage: /tui input [panel|separator]",
                    "用法: /tui input [panel|separator]",
                )
                .into(),
            )?;
            return Ok(true);
        };
        // 1. 【输入样式】【偏好保存】先保存磁盘配置，失败时不改变当前界面
        if let Some(paths) = &self.session_browser.paths {
            let result = (|| {
                let mut config = AppConfig::load_or_default(paths)?;
                config.display.input_style = style;
                config.save(paths)
            })();
            if let Err(error) = result {
                self.record_meta(error.to_string())?;
                return Ok(true);
            }
        }
        // 2. 【输入样式】【即时展示】保留草稿和选择，仅更新样式后重绘
        self.input_style.selected = Some(style);
        self.configure_input_style(self.input_style.default);
        self.redraw()?;
        self.record_meta(format!("input: {}", style.as_str()))?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【输入样式测试】【偏好保存】参数：无；返回：无，验证命令持久化、默认配置不覆盖手动选择
    #[test]
    fn input_style_command_persists_and_preserves_manual_selection() {
        let root = tempfile::tempdir().unwrap();
        let paths = crate::paths::SaiPaths::for_tests(root.path());
        let state = crate::state::StateStore::new(&paths).unwrap();
        let mut runtime = ReplRuntime::new(
            1000,
            crate::render::transcript::TranscriptRenderOptions {
                reasoning_mode: crate::render::ReasoningDisplayMode::Summary,
                tool_call_mode: crate::render::ToolCallDisplayMode::Summary,
            },
        );
        runtime.bind_session_browser(&paths, &state);
        assert!(runtime.handle_tui_command("/tui input separator").unwrap());
        assert_eq!(
            AppConfig::load(&paths).unwrap().display.input_style,
            InputStyle::Separator
        );
        runtime.configure_input_style(InputStyle::Panel);
        assert_eq!(runtime.input_style.current(), InputStyle::Separator);
        runtime.handle_tui_command("/tui input invalid").unwrap();
        assert_eq!(runtime.input_style.current(), InputStyle::Separator);
        runtime.handle_tui_command("/tui input panel").unwrap();
        assert_eq!(
            AppConfig::load(&paths).unwrap().display.input_style,
            InputStyle::Panel
        );
        assert_eq!(runtime.queue_len(), 0);
    }
}
