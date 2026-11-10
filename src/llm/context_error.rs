/// 【模型接口】【上下文溢出】识别供应商上下文容量错误，供所有执行器使用。
/// 参数：请求错误；返回是否允许压缩后重试一次。
pub(crate) fn is_context_overflow_error(err: &anyhow::Error) -> bool {
    let message = format!("{err:#}").to_ascii_lowercase();
    [
        "context_length_exceeded",
        "maximum context",
        "context window",
        "context length",
        "too many tokens",
        "tokens exceed",
        "prompt is too long",
        "input is too long",
    ]
    .iter()
    .any(|needle| message.contains(needle))
}
