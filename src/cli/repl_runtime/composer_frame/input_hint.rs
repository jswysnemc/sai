use crate::render::terminal_text as t;

/// 【终端】【输入引导】按工作状态和宽度提供明确的输入动作，不重复底栏快捷键。
/// 参数: `streaming` 为是否正在工作，`width` 为净宽；返回: 适合当前宽度的提示，宽度不足时为空。
pub(super) fn placeholder(streaming: bool, width: usize) -> &'static str {
    let candidates = if streaming {
        [
            t("Add a follow-up to the queue", "输入后续消息，加入队列"),
            t("Queue a follow-up", "输入后续消息"),
            t("Follow-up", "后续消息"),
        ]
    } else {
        [
            t("Enter a message or type /help", "输入消息，或使用 /help"),
            t("Enter a message", "输入消息"),
            t("Message", "消息"),
        ]
    };
    candidates
        .into_iter()
        .find(|text| unicode_width::UnicodeWidthStr::width(*text) <= width)
        .unwrap_or("")
}

#[cfg(test)]
#[path = "input_hint_tests.rs"]
mod tests;
