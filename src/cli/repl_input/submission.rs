use super::*;
use crate::agent::ExternalEventWake;
use crate::render::input_atom::InputEcho;

pub(in crate::cli) struct ReplInputSubmission {
    pub(in crate::cli) mode: AgentMode,
    pub(in crate::cli) raw_input: String,
    pub(in crate::cli) chat_input: clipboard::ClipboardChatInput,
    /// 提交回显的完整正文与原子块元数据
    pub(in crate::cli) echo: InputEcho,
}

impl ReplInputSubmission {
    /// 【终端】【输入提交】在清空输入前冻结正文、附件和回显元数据。
    ///
    /// 参数: `mode` 为提交模式，`raw_input` 为输入框内容，`clipboard` 为已登记附件
    /// 返回: 可交主循环或队列执行的完整提交
    pub(in crate::cli) fn from_input(
        mode: AgentMode,
        raw_input: String,
        clipboard: &ReplClipboardState,
    ) -> Self {
        Self {
            mode,
            chat_input: clipboard.to_chat_input(&raw_input),
            echo: clipboard.echo_text_for_submit(&raw_input),
            raw_input,
        }
    }

    /// 构造一条来自控制队列的提交。
    ///
    /// 运行期间输入的斜杠命令没有经过输入框，这里补齐主循环分发所需的形状；
    /// 正文不会发给模型，回显也由调用方单独记录，因此 echo 字段留空。
    ///
    /// 参数:
    /// - `mode`: 当前 Agent 模式
    /// - `command`: 命令原文
    ///
    /// 返回:
    /// - 可交主循环分发的提交
    pub(in crate::cli) fn control(mode: AgentMode, command: String) -> Self {
        Self {
            mode,
            raw_input: command,
            chat_input: clipboard::ClipboardChatInput {
                message: String::new(),
                image_url: None,
            },
            echo: InputEcho::default(),
        }
    }
}

/// 输入框产生的下一项工作。
pub(in crate::cli) enum ReplInputEvent {
    User(ReplInputSubmission),
    Automatic {
        mode: AgentMode,
        wake: ExternalEventWake,
        draft: ReplInputDraft,
    },
}

/// 自动唤醒期间暂存的输入文本与剪贴板附件。
pub(in crate::cli) struct ReplInputDraft {
    pub(in crate::cli) text: String,
    pub(in crate::cli) clipboard_state: ReplClipboardState,
}
