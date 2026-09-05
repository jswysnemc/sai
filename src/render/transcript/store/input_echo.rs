use super::*;
use crate::render::input_atom::InputEcho;
use crate::render::transcript::cell::TranscriptMode;
use crate::render::transcript::user_echo_cell::UserEchoCell;

impl TranscriptStore {
    /// 记录用户输入回显。
    ///
    /// 参数:
    /// - `mode`: 用户提交时的 REPL 模式
    /// - `text`: 原始输入文本
    ///
    /// 返回:
    /// - 无
    pub(crate) fn push_user_echo(&mut self, mode: TranscriptMode, text: String) {
        self.push_user_echo_with_fold(mode, text, false);
    }

    /// 记录用户输入回显；`fold` 仅对粘贴长文本启用思考式折叠。
    ///
    /// 参数:
    /// - `mode`: 用户提交时的 REPL 模式
    /// - `text`: 回显正文（粘贴块应已展开）
    /// - `fold`: 是否按思考块语义折叠
    ///
    /// 返回:
    /// - 无
    pub(crate) fn push_user_echo_with_fold(
        &mut self,
        mode: TranscriptMode,
        text: String,
        fold: bool,
    ) {
        let cell = if fold {
            HistoryCell::user_echo_with_fold(mode, text, true)
        } else {
            HistoryCell::user_echo(mode, text)
        };
        self.push_cell(cell);
    }

    /// 记录 Sai 主动提交的自动输入回显。
    ///
    /// 参数:
    /// - `text`: 展示给用户的自动消息文本
    ///
    /// 返回:
    /// - 无
    pub(crate) fn push_automatic_echo(&mut self, text: String) {
        self.push_user_echo(TranscriptMode::Automatic, text);
    }

    /// 【终端】【输入回显】保存完整正文与原子块来源。
    ///
    /// 参数: `mode` 为提交模式，`echo` 为提交回显数据
    /// 返回: 无
    pub(crate) fn push_user_input(&mut self, mode: TranscriptMode, echo: InputEcho) {
        self.push_cell(HistoryCell::UserEcho(UserEchoCell::with_atoms(mode, echo)));
    }
}
