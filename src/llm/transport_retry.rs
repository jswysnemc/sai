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

/// 用户可见的断连提示（可手动重试）。
///
/// 参数:
/// - `error`: 原始错误
///
/// 返回:
/// - 面向用户的摘要文本；完整错误链请用 `error_detail_text`
pub(crate) fn disconnect_user_hint(error: &anyhow::Error) -> String {
    use crate::i18n::text as t;
    if is_transient_transport_error(error) {
        format!(
            "{}: {}\n{}",
            t("Connection interrupted", "连接中断"),
            simplify_one_line(error),
            t(
                "You can retry this turn.",
                "可重试本轮请求。",
            )
        )
    } else {
        simplify_one_line(error)
    }
}

/// 返回适合 UI 详情区展示的完整错误链。
///
/// 参数:
/// - `error`: 原始错误
///
/// 返回:
/// - 去重后的完整错误链文本
pub(crate) fn error_detail_text(error: &anyhow::Error) -> String {
    let mut lines = Vec::new();
    for item in error.chain() {
        let text = item.to_string();
        let text = text.trim();
        if text.is_empty() {
            continue;
        }
        if lines.last().map(String::as_str) == Some(text) {
            continue;
        }
        lines.push(text.to_string());
    }
    if lines.is_empty() {
        return error.to_string();
    }
    lines.join("\n")
}

/// 提取单行简化错误。
fn simplify_one_line(error: &anyhow::Error) -> String {
    error
        .chain()
        .next()
        .map(|item| item.to_string())
        .unwrap_or_else(|| error.to_string())
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
        assert_ne!(disconnect_user_hint(&err), detail);
    }
}
