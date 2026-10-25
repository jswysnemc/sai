use super::*;
use crate::render::input_atom::InputEcho;
use crate::render::transcript::cell::TranscriptMode;
use crate::render::transcript::user_echo_cell::UserEchoCell;

impl TranscriptStore {
    /// 【终端】【会话通知】后续消息按完成的工具条目入列，保留图片来源但不增加导航点。
    /// 参数: `kind` 为消息来源，`text` 为消息正文，`sources` 为图片来源；返回: 无。
    pub(crate) fn push_conversation_notice(
        &mut self,
        kind: crate::render::ConversationNoticeKind,
        text: String,
        sources: &[String],
    ) {
        let arguments =
            serde_json::json!({"kind":kind.as_str(), "image_sources":sources}).to_string();
        let mut view =
            crate::render::tool_view::ToolView::running("conversation_notice".into(), arguments);
        view.finish(true, text);
        self.push_cell(HistoryCell::Tool(
            crate::render::transcript::tool_cell::ToolCell::Invocation(Box::new(view)),
        ));
    }

    /// 【终端】【后台通知】只记录工具样式通知，不增加用户消息或会话导航锚点。
    /// 参数: `text` 为自动完成通知；返回: 无。
    pub(crate) fn push_background_notice(&mut self, text: String) {
        let mut view =
            crate::render::tool_view::ToolView::running("background_notice".into(), String::new());
        view.finish(true, text);
        self.push_cell(HistoryCell::Tool(
            crate::render::transcript::tool_cell::ToolCell::Invocation(Box::new(view)),
        ));
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    /// 【终端】【通知生命周期测试】通知只追加展示单元，不覆盖正在执行的工具或工作计时。
    /// 参数: 无；返回: 无。
    #[test]
    fn background_notice_keeps_the_active_tool_and_work_clock() {
        let mut store = TranscriptStore::new(200);
        store.push_tool_call("run_command".into(), r#"{"command":"echo result"}"#.into());
        let active = store.active_tool_index;
        let started = store.work_status_started;
        store.push_background_notice("Background work completed".into());
        assert_eq!(store.active_tool_index, active);
        assert_eq!(store.work_status_started, started);
        store.push_tool_result("run_command".into(), true, "result".into());
        assert!(
            matches!(&store.cells[0], HistoryCell::Tool(crate::render::transcript::tool_cell::ToolCell::Invocation(view)) if view.outcome.as_ref().is_some_and(|outcome| outcome.output == "result"))
        );
    }
}
