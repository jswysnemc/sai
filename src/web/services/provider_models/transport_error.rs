//! 网络传输错误的根因展开。
//!
//! reqwest 0.12 的 `Display` 只输出最外层描述（例如 `error sending request for url (...)`），
//! 真正的原因藏在 `source()` 链里。直接 `to_string()` 会把 DNS、连接被拒、证书失效
//! 这些互不相同的故障压成同一句话，界面上无从判断，错误分类也只能归为未知。

use std::error::Error;

/// 展开传输错误的完整根因链路。
///
/// 参数:
/// - `error`: 发送请求时产生的错误
///
/// 返回:
/// - 由外到内拼接的错误描述，形如 `外层: 中层: 根因`
pub(super) fn describe_transport_error(error: &reqwest::Error) -> String {
    describe_error_chain(error)
}

/// 遍历任意错误的 source 链并拼接为单行文本。
///
/// 参数:
/// - `error`: 任意实现了 `std::error::Error` 的错误
///
/// 返回:
/// - 去除相邻重复后的错误链路文本
fn describe_error_chain(error: &dyn Error) -> String {
    let mut segments: Vec<String> = Vec::new();
    let mut current: Option<&dyn Error> = Some(error);
    // 1. 逐层取出 source，直到链路末端
    while let Some(item) = current {
        let text = item.to_string();
        let trimmed = text.trim();
        // 2. 跳过空描述与紧邻的重复描述，避免出现同一句话叠加多次
        if !trimmed.is_empty() && segments.last().map(String::as_str) != Some(trimmed) {
            segments.push(trimmed.to_string());
        }
        current = item.source();
    }
    if segments.is_empty() {
        return "unknown transport error".to_string();
    }
    segments.join(": ")
}

#[cfg(test)]
mod tests {
    use super::describe_error_chain;
    use std::error::Error;
    use std::fmt;

    #[derive(Debug)]
    struct TestError {
        message: String,
        source: Option<Box<TestError>>,
    }

    impl fmt::Display for TestError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(&self.message)
        }
    }

    impl Error for TestError {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            self.source
                .as_ref()
                .map(|item| item.as_ref() as &(dyn Error + 'static))
        }
    }

    /// 构造一层测试错误。
    fn error(message: &str, source: Option<TestError>) -> TestError {
        TestError {
            message: message.to_string(),
            source: source.map(Box::new),
        }
    }

    /// 验证根因被拼接进最终文本，而不是只保留最外层描述。
    #[test]
    fn joins_root_cause_into_description() {
        let chain = error(
            "error sending request for url (https://api.example.test/v1/models)",
            Some(error(
                "client error (Connect)",
                Some(error("dns error: failed to lookup address", None)),
            )),
        );

        assert_eq!(
            describe_error_chain(&chain),
            "error sending request for url (https://api.example.test/v1/models): \
client error (Connect): dns error: failed to lookup address"
        );
    }

    /// 验证相邻重复描述只保留一次。
    #[test]
    fn collapses_adjacent_duplicate_messages() {
        let chain = error(
            "connection closed",
            Some(error("connection closed", Some(error("broken pipe", None)))),
        );

        assert_eq!(
            describe_error_chain(&chain),
            "connection closed: broken pipe"
        );
    }

    /// 验证无描述的错误链退回固定文案而不是返回空串。
    #[test]
    fn falls_back_when_chain_has_no_text() {
        let chain = error("   ", None);

        assert_eq!(describe_error_chain(&chain), "unknown transport error");
    }
}
