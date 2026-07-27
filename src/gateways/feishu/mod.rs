mod client;
mod event;
mod server;

pub(crate) use server::{run_feishu_bot_server, FeishuBotServerConfig};

/// 返回飞书渠道的系统提示词。
///
/// 群聊里回复要短、可直接读；飞书的文本消息不渲染 Markdown，
/// 因此明确要求不要输出表格与代码块围栏。
///
/// 返回:
/// - 渠道系统提示词
pub(crate) fn channel_prompt() -> &'static str {
    crate::i18n::text(
        "You are replying inside a Feishu chat. Keep answers short and directly readable. Feishu text messages do not render Markdown, so avoid tables and code fences; use plain lines instead.",
        "你正在飞书会话中回复。回答要简短、可直接阅读。飞书文本消息不渲染 Markdown，请避免表格与代码围栏，改用纯文本分行。",
    )
}
