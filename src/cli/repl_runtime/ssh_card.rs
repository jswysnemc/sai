use super::ReplRuntime;
use crate::cli::ssh_prompt::SshPromptView;
use anyhow::Result;

impl ReplRuntime {
    /// 【SSH 征询】【TUI 卡片】挂载或更新输入框上方的征询卡片并立即重绘。
    ///
    /// 卡片走 composer 的沉底面板通道，由受管区域统一腾行与清理，
    /// 不再向光标处直接写字而覆盖输入框和底栏。
    ///
    /// 参数:
    /// - `view`: 征询卡片视图（不含秘密）
    ///
    /// 返回:
    /// - 重绘结果
    pub(in crate::cli) fn show_ssh_prompt(&mut self, view: SshPromptView) -> Result<()> {
        if self.ssh_prompt.as_ref() == Some(&view) {
            return Ok(());
        }
        self.ssh_prompt = Some(view);
        self.redraw_stream_composer()
    }

    /// 【SSH 征询】【TUI 卡片】撤下征询卡片，恢复运行期间的输入草稿。
    ///
    /// 返回:
    /// - 重绘结果
    pub(in crate::cli) fn clear_ssh_prompt(&mut self) -> Result<()> {
        if self.ssh_prompt.take().is_none() {
            return Ok(());
        }
        self.redraw_stream_composer()
    }

    /// 返回当前征询卡片行；无征询时为空。
    ///
    /// 参数:
    /// - `cols`: 终端列数
    ///
    /// 返回:
    /// - 卡片 ANSI 行
    pub(super) fn ssh_card_lines(&self, cols: usize) -> Vec<String> {
        self.ssh_prompt
            .as_ref()
            .map(|view| view.panel_lines(cols))
            .unwrap_or_default()
    }

    /// 把征询占位提示写入当前 composer，必须在 update_composer 之后调用。
    ///
    /// 返回:
    /// - 无
    pub(super) fn apply_ssh_placeholder(&mut self) {
        let placeholder = self
            .ssh_prompt
            .as_ref()
            .map(|view| view.placeholder().to_string());
        if let Some(composer) = self.composer.as_mut() {
            composer.set_placeholder_override(placeholder);
        }
    }
}
