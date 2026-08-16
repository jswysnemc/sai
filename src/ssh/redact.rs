//! SSH 秘密脱敏红线。
//!
//! 所有由后端持有的秘密（私钥口令、登录密码等）在任何时候都不得进入模型上下文、
//! 工具结果、错误消息、日志或 transcript。本模块提供统一的脱敏入口：工具在把结果
//! 或错误交还给上层之前，先用本次连接实际用到的秘密值跑一遍替换，确保即使远端命令
//! 回显了口令，也只会看到脱敏占位。

/// 模型可见的脱敏占位符。
pub(crate) const SECRET_PLACEHOLDER: &str = "<secret redacted>";

/// 触发脱敏的最短秘密长度。
///
/// 过短的值（例如单字符）在正常文本里高频出现，一律替换会把无关内容也抹掉，
/// 反而破坏可读性且暴露"这里曾有秘密"的位置。真实口令/密码远长于此阈值。
const MIN_REDACTABLE_LEN: usize = 3;

/// 把文本中出现的秘密值替换为脱敏占位符。
///
/// 参数:
/// - `text`: 待脱敏文本（命令输出、错误消息等）
/// - `secrets`: 本次操作实际使用过的秘密明文集合
///
/// 返回:
/// - 所有命中秘密均被替换为占位符的文本
pub(crate) fn redact(text: &str, secrets: &[String]) -> String {
    let mut redacted = text.to_string();
    for secret in secrets {
        let secret = secret.as_str();
        // 空值与过短值不参与替换，避免误伤正常文本
        if secret.chars().count() < MIN_REDACTABLE_LEN {
            continue;
        }
        if redacted.contains(secret) {
            redacted = redacted.replace(secret, SECRET_PLACEHOLDER);
        }
    }
    redacted
}

/// 对错误进行脱敏并转为面向模型的字符串。
///
/// 错误链里可能间接带上认证细节，返回给模型前统一走脱敏。
///
/// 参数:
/// - `error`: 原始错误
/// - `secrets`: 本次操作使用过的秘密明文集合
///
/// 返回:
/// - 脱敏后的错误文本
pub(crate) fn redact_error(error: &anyhow::Error, secrets: &[String]) -> String {
    redact(&format!("{error:#}"), secrets)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_a_known_secret() {
        let text = "connecting with password hunter2 to host";
        let redacted = redact(text, &["hunter2".to_string()]);
        assert_eq!(
            redacted,
            "connecting with password <secret redacted> to host"
        );
        assert!(!redacted.contains("hunter2"));
    }

    #[test]
    fn replaces_every_occurrence_of_multiple_secrets() {
        let text = "pass=topsecret key-pass=keyphrase again topsecret";
        let redacted = redact(text, &["topsecret".to_string(), "keyphrase".to_string()]);
        assert!(!redacted.contains("topsecret"));
        assert!(!redacted.contains("keyphrase"));
        assert_eq!(redacted.matches(SECRET_PLACEHOLDER).count(), 3);
    }

    #[test]
    fn ignores_empty_and_too_short_secrets() {
        let text = "a normal line with a and ab tokens";
        let redacted = redact(text, &[String::new(), "a".to_string(), "ab".to_string()]);
        // 过短秘密不替换，正常文本保持不变
        assert_eq!(redacted, text);
    }

    #[test]
    fn leaves_unrelated_text_untouched() {
        let text = "total 8\ndrwxr-xr-x 2 user user 4096 app";
        let redacted = redact(text, &["hunter2".to_string()]);
        assert_eq!(redacted, text);
    }

    #[test]
    fn redacts_secret_inside_error_chain() {
        let error = anyhow::anyhow!("auth failed with passphrase s3cretphrase");
        let redacted = redact_error(&error, &["s3cretphrase".to_string()]);
        assert!(!redacted.contains("s3cretphrase"));
        assert!(redacted.contains(SECRET_PLACEHOLDER));
    }
}
