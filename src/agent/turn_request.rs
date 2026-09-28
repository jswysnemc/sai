use super::InterMessageSource;

/// 【会话轮次】【请求上下文】工具执行与压缩恢复共享同一份输入和提示，避免重建时参数漂移。
#[derive(Clone, Copy)]
pub(super) struct TurnRequest<'a> {
    pub turn_id: &'a str,
    pub input: &'a str,
    pub image_urls: &'a [String],
    pub memory_index_prompt: Option<&'a str>,
    pub plugin_reply_reminder: Option<&'a str>,
    pub inter_message_source: Option<&'a dyn InterMessageSource>,
    pub wait_for_external: bool,
}
