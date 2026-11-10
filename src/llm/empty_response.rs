use super::ChatResult;

/// 【模型接口】【空响应】保留已完成但无正文响应的用量，交由目标运行时决定是否续轮
#[derive(Debug)]
pub(crate) struct EmptyModelResponse(pub(crate) ChatResult);

impl std::fmt::Display for EmptyModelResponse {
    /// 【模型接口】【错误展示】参数为格式器；返回错误文本写入结果
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(crate::i18n::text(
            "chat completions stream response was empty",
            "聊天流式响应为空",
        ))
    }
}

impl std::error::Error for EmptyModelResponse {}
