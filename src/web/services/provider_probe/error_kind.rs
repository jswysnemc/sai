/// 探测失败的归类。
///
/// 归类的意义在于给出可操作的下一步：网络不通要查地址与代理，
/// 凭据无效要换密钥，模型不存在要改模型名。只回一句"失败"没有诊断价值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeErrorKind {
    /// 连不上：DNS、拒绝连接、代理故障
    Network,
    /// 超时
    Timeout,
    /// 凭据无效或权限不足
    Auth,
    /// 地址或模型标识不存在
    NotFound,
    /// 触发限流
    RateLimit,
    /// 上游服务端错误
    Server,
    /// 能连通但响应无法按协议解析
    Protocol,
    /// 未能归类
    Unknown,
}

impl ProbeErrorKind {
    /// 返回稳定的字符串标识，供前端匹配文案。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 小写下划线形式的标识
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Network => "network",
            Self::Timeout => "timeout",
            Self::Auth => "auth",
            Self::NotFound => "not_found",
            Self::RateLimit => "rate_limit",
            Self::Server => "server",
            Self::Protocol => "protocol",
            Self::Unknown => "unknown",
        }
    }
}

/// 从错误文本推断失败类型。
///
/// 底层各协议的错误都归并成了字符串，这里按特征词还原类别。
/// 判定顺序从具体到宽泛：状态码优先，其次是各家 API 的错误短语。
///
/// 参数:
/// - `message`: 错误信息全文
///
/// 返回:
/// - 推断出的失败类型
pub fn classify(message: &str) -> ProbeErrorKind {
    let text = message.to_ascii_lowercase();
    // 1. 传输层问题优先判定：这类错误根本没拿到 HTTP 状态码
    if text.contains("timed out") || text.contains("timeout") || text.contains("deadline") {
        return ProbeErrorKind::Timeout;
    }
    if text.contains("dns")
        || text.contains("connection refused")
        || text.contains("connect error")
        || text.contains("failed to connect")
        || text.contains("tcp connect")
        || text.contains("certificate")
        || text.contains("tls")
    {
        return ProbeErrorKind::Network;
    }
    // 2. 状态码与其对应的常见短语
    if text.contains("401") || text.contains("403") || contains_any(&text, AUTH_PHRASES) {
        return ProbeErrorKind::Auth;
    }
    if text.contains("429") || text.contains("rate limit") || text.contains("quota") {
        return ProbeErrorKind::RateLimit;
    }
    if text.contains("404") || contains_any(&text, NOT_FOUND_PHRASES) {
        return ProbeErrorKind::NotFound;
    }
    if contains_any(&text, SERVER_STATUS) {
        return ProbeErrorKind::Server;
    }
    // 3. 能通信但解析不了，多半是地址指向了非兼容端点
    if text.contains("expected value")
        || text.contains("invalid json")
        || text.contains("missing field")
        || text.contains("unexpected")
        || text.contains("decode")
    {
        return ProbeErrorKind::Protocol;
    }
    ProbeErrorKind::Unknown
}

const AUTH_PHRASES: &[&str] = &[
    "unauthorized",
    "invalid api key",
    "invalid_api_key",
    "authentication",
    "permission denied",
    "forbidden",
];

const NOT_FOUND_PHRASES: &[&str] = &[
    "not found",
    "does not exist",
    "unknown model",
    "model_not_found",
    "no such model",
];

const SERVER_STATUS: &[&str] = &["500", "502", "503", "504", "bad gateway", "server error"];

/// 判断文本是否包含候选短语之一。
///
/// 参数:
/// - `text`: 已转小写的待检文本
/// - `phrases`: 候选短语
///
/// 返回:
/// - 命中任一短语时为 true
fn contains_any(text: &str, phrases: &[&str]) -> bool {
    phrases.iter().any(|phrase| text.contains(phrase))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证凭据类错误被正确归类。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无；断言失败则测试不通过
    #[test]
    fn classifies_auth_failures() {
        assert_eq!(classify("HTTP 401 Unauthorized"), ProbeErrorKind::Auth);
        assert_eq!(classify("invalid api key provided"), ProbeErrorKind::Auth);
    }

    /// 验证传输层错误优先于状态码判定。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无；断言失败则测试不通过
    #[test]
    fn prefers_transport_errors() {
        assert_eq!(
            classify("connection refused (os error 111)"),
            ProbeErrorKind::Network
        );
        assert_eq!(
            classify("request timed out after 30s"),
            ProbeErrorKind::Timeout
        );
        // 展开后的 reqwest 链路把 DNS 根因露出来，才能从「未知」归到网络
        assert_eq!(
            classify(
                "error sending request for url (https://api.example.test/v1/models): \
client error (Connect): dns error: failed to lookup address"
            ),
            ProbeErrorKind::Network
        );
    }

    /// 验证模型不存在与限流的归类。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无；断言失败则测试不通过
    #[test]
    fn classifies_model_and_rate_limit_failures() {
        assert_eq!(classify("model_not_found: gpt-9"), ProbeErrorKind::NotFound);
        assert_eq!(
            classify("HTTP 429 rate limit exceeded"),
            ProbeErrorKind::RateLimit
        );
    }

    /// 验证无法解析的响应归为协议问题。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无；断言失败则测试不通过
    #[test]
    fn classifies_unparsable_response_as_protocol() {
        assert_eq!(
            classify("expected value at line 1 column 1"),
            ProbeErrorKind::Protocol
        );
    }
}
