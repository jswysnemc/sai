use std::time::Duration;

/// 【模型请求】【HTTP 错误】保留协议状态和服务器建议的重试间隔
#[derive(Debug)]
pub(crate) struct ProviderHttpError {
    pub(crate) status: u16,
    pub(crate) retry_after: Option<Duration>,
    pub(crate) message: String,
}

impl std::fmt::Display for ProviderHttpError {
    /// 【模型请求】【错误展示】参数为格式器；返回格式化结果
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "model request failed ({}): {}",
            self.status, self.message
        )
    }
}
impl std::error::Error for ProviderHttpError {}

/// 【模型请求】【服务端退避】解析秒数或 HTTP 日期；参数为响应头，返回等待时间
pub(crate) fn retry_header(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    let value = headers.get(reqwest::header::RETRY_AFTER)?.to_str().ok()?;
    if let Ok(seconds) = value.trim().parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }
    let date = chrono::DateTime::parse_from_rfc2822(value).ok()?;
    (date.with_timezone(&chrono::Utc) - chrono::Utc::now())
        .to_std()
        .ok()
        .or(Some(Duration::ZERO))
}

/// 【模型请求】【错误链退避】读取结构化错误中的服务端等待时间；参数为错误，返回间隔
pub(crate) fn retry_after(error: &anyhow::Error) -> Option<Duration> {
    error
        .downcast_ref::<ProviderHttpError>()
        .and_then(|error| error.retry_after)
}

/// 【模型请求】【中断类型】明确表示尚未获得协议结束标记
#[derive(Debug)]
pub(crate) struct IncompleteStream(pub(crate) String);
impl std::fmt::Display for IncompleteStream {
    /// 【模型请求】【中断说明】参数为格式器；返回错误说明
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} stream ended before the upstream signalled completion",
            self.0
        )
    }
}
impl std::error::Error for IncompleteStream {}
