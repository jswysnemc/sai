use serde_json::Value;

/// 判断错误是否适合在模型输出开始前自动重试（网络/断连/瞬时网关）。
///
/// 参数:
/// - `error`: 原始错误
///
/// 返回:
/// - 可自动重试则为 true
pub(crate) fn is_transient_transport_error(error: &anyhow::Error) -> bool {
    error.chain().any(|item| {
        let message = item.to_string().to_ascii_lowercase();
        is_transient_message(&message)
    })
}

/// 判断错误文本是否为瞬时传输故障。
///
/// 参数:
/// - `message`: 小写错误文本
///
/// 返回:
/// - 是否瞬时故障
fn is_transient_message(message: &str) -> bool {
    // HTTP 4xx 业务错误不重试（401/402/403/404/422 等）
    if [400u16, 401, 402, 403, 404, 422]
        .iter()
        .any(|status| contains_status(message, *status))
        || message.contains("invalid_request")
        || message.contains("insufficient balance")
        || message.contains("context_length")
        || message.contains("context window")
    {
        return false;
    }
    if [408u16, 425, 429, 500, 502, 503, 504]
        .iter()
        .any(|status| contains_status(message, *status))
    {
        return true;
    }
    let needles = [
        "connection reset",
        "connection closed",
        "connection refused",
        "broken pipe",
        "timed out",
        "timeout",
        "temporarily unavailable",
        "temporary failure",
        "network is unreachable",
        "dns error",
        "name or service not known",
        "failed to connect",
        "error sending request",
        "error decoding response body",
        "unexpected eof",
        "handshake eof",
        "tls handshake",
        "ssl handshake",
        "client error",
        "connection error",
        "stream closed",
        "http2 error",
        "goaway",
        "reset by peer",
        "bad gateway",
        "service unavailable",
        "gateway timeout",
    ];
    needles.iter().any(|needle| message.contains(needle))
}

/// 判断错误文本中是否携带指定 HTTP 状态码。
///
/// `reqwest::StatusCode` 的 Display 会附带标准原因短语，因此错误文本里出现的是
/// `(502 bad gateway)` 而不是 `(502)`。这里同时接受两种形式，避免状态码判定
/// 依赖原因短语是否存在。
///
/// 参数:
/// - `message`: 小写错误文本
/// - `status`: 目标 HTTP 状态码
///
/// 返回:
/// - 文本中出现该状态码时返回 true
fn contains_status(message: &str, status: u16) -> bool {
    message.contains(&format!("({status})")) || message.contains(&format!("({status} "))
}

/// 返回适合 UI 详情区展示的完整错误链。
///
/// 参数:
/// - `error`: 原始错误
///
/// 返回:
/// - 去重后的完整错误链文本
pub(crate) fn error_detail_text(error: &anyhow::Error) -> String {
    let mut lines: Vec<String> = Vec::new();
    for item in error.chain() {
        let text = simplify_error_text(item.to_string().trim());
        if text.is_empty() {
            continue;
        }
        if lines.last() == Some(&text) {
            continue;
        }
        lines.push(text);
    }
    if lines.is_empty() {
        return error.to_string();
    }
    lines.join("\n")
}

/// 把供应商返回的 JSON 错误体收成可读短句。
///
/// 参数:
/// - `text`: 原始错误文本
///
/// 返回:
/// - 适合终端展示的错误文本
pub(crate) fn simplify_error_text(text: &str) -> String {
    // 1. 没有 JSON 错误体时原样返回
    let Some((prefix, body)) = text.split_once(": {") else {
        return text.to_string();
    };
    // 2. 解析供应商 error 对象
    let body = format!("{{{body}");
    let Ok(value) = serde_json::from_str::<Value>(&body) else {
        return text.to_string();
    };
    let Some(error) = value.get("error") else {
        return text.to_string();
    };
    // 3. 抽出 message / type / code 拼成短句
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("unknown provider error");
    let kind = error.get("type").and_then(Value::as_str);
    let code = error.get("code").and_then(Value::as_str);
    let mut details = Vec::new();
    if let Some(kind) = kind.filter(|value| !value.is_empty()) {
        details.push(format!("type={kind}"));
    }
    if let Some(code) = code.filter(|value| !value.is_empty()) {
        details.push(format!("code={code}"));
    }
    if details.is_empty() {
        format!("{prefix}: {message}")
    } else {
        format!("{prefix}: {message} ({})", details.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_timeouts_as_transient() {
        let err = anyhow::anyhow!("error sending request for url: operation timed out");
        assert!(is_transient_transport_error(&err));
    }

    #[test]
    fn classifies_auth_errors_as_permanent() {
        let err = anyhow::anyhow!("chat completions stream request failed (401): invalid key");
        assert!(!is_transient_transport_error(&err));
    }

    #[test]
    fn classifies_502_as_transient() {
        let err = anyhow::anyhow!("chat completions stream request failed (502): bad gateway");
        assert!(is_transient_transport_error(&err));
    }

    /// 【协议】【重试判定】验证带原因短语的状态码同样被识别。
    ///
    /// reqwest 的 StatusCode Display 会附带标准原因短语，真实错误文本里是
    /// `(502 Bad Gateway)` 而非 `(502)`。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn recognizes_status_codes_with_reason_phrases() {
        for message in [
            "chat completions stream request failed (500 internal server error): oops",
            "chat completions stream request failed (429 too many requests): slow down",
            "chat completions stream request failed (503 service unavailable)",
        ] {
            let err = anyhow::anyhow!("{message}");
            assert!(
                is_transient_transport_error(&err),
                "应判定为可重试: {message}"
            );
        }
    }

    /// 【协议】【重试判定】验证业务错误即使含瞬时关键词也不重试。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn keeps_client_errors_permanent_despite_transient_words() {
        for message in [
            "chat completions stream request failed (400 bad request): timeout must be positive",
            "chat completions stream request failed (422 unprocessable entity): connection reset is not a field",
        ] {
            let err = anyhow::anyhow!("{message}");
            assert!(
                !is_transient_transport_error(&err),
                "不应判定为可重试: {message}"
            );
        }
    }

    #[test]
    fn error_detail_text_keeps_full_chain() {
        let err = anyhow::anyhow!("root cause")
            .context("mid layer")
            .context("outer layer");
        let detail = error_detail_text(&err);
        assert!(detail.contains("outer layer"));
        assert!(detail.contains("mid layer"));
        assert!(detail.contains("root cause"));
    }

    #[test]
    fn error_detail_text_extracts_openai_error_body() {
        let err = anyhow::anyhow!(
            "chat completions stream request failed (400 Bad Request): {{\"error\": {{\"message\": \"QUEUE-FAILURE-FIXTURE\", \"type\": \"invalid_request_error\"}}}}"
        );
        let detail = error_detail_text(&err);
        assert!(detail.contains("QUEUE-FAILURE-FIXTURE"));
        assert!(!detail.contains(r#""message""#));
    }
}
