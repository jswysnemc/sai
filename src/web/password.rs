use anyhow::{Context, Result};
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier};
use argon2::Argon2;

/// 口令最短长度，低于此长度的口令在设置时被拒绝。
pub(crate) const MIN_WEB_PASSWORD_LENGTH: usize = 8;

/// 计算 Web 访问口令的存储哈希。
///
/// 使用 Argon2id 加随机盐：口令哈希随配置落盘，必须能抵抗离线暴力枚举，
/// 单轮 SHA 之类的通用摘要不满足这一点。盐由 argon2 自动生成。
///
/// 参数:
/// - `password`: 明文口令
///
/// 返回:
/// - PHC 字符串格式的哈希
pub(crate) fn hash_web_password(password: &str) -> Result<String> {
    if password.chars().count() < MIN_WEB_PASSWORD_LENGTH {
        anyhow::bail!("password must be at least {MIN_WEB_PASSWORD_LENGTH} characters");
    }
    Ok(Argon2::default()
        .hash_password(password.as_bytes())
        .map_err(|error| anyhow::anyhow!("{error}"))?
        .to_string())
}

/// 校验口令是否与存储的哈希匹配。
///
/// 比较由 argon2 内部以恒定时间完成，不会因失败位置提前返回。
///
/// 参数:
/// - `password`: 待校验的明文口令
/// - `stored`: PHC 字符串格式的哈希
///
/// 返回:
/// - 口令是否正确
pub(crate) fn verify_web_password(password: &str, stored: &str) -> Result<bool> {
    let parsed = PasswordHash::new(stored)
        .map_err(|error| anyhow::anyhow!("{error}"))
        .context("stored password hash is malformed")?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_the_original_password() {
        let hash = hash_web_password("correct horse").expect("应能计算口令哈希");
        assert!(verify_web_password("correct horse", &hash).unwrap());
    }

    #[test]
    fn rejects_a_wrong_password() {
        let hash = hash_web_password("correct horse").expect("应能计算口令哈希");
        assert!(!verify_web_password("correct horsE", &hash).unwrap());
        assert!(!verify_web_password("", &hash).unwrap());
    }

    #[test]
    fn produces_a_distinct_hash_per_call() {
        // 每次使用新盐，相同口令的哈希不应相同，否则可据此比对用户复用情况
        let first = hash_web_password("correct horse").unwrap();
        let second = hash_web_password("correct horse").unwrap();
        assert_ne!(first, second);
        assert!(verify_web_password("correct horse", &first).unwrap());
        assert!(verify_web_password("correct horse", &second).unwrap());
    }

    #[test]
    fn stores_the_hash_rather_than_the_password() {
        let hash = hash_web_password("correct horse").unwrap();
        assert!(!hash.contains("correct horse"));
        assert!(hash.starts_with("$argon2"));
    }

    #[test]
    fn rejects_passwords_below_the_minimum_length() {
        assert!(hash_web_password("short").is_err());
        assert!(hash_web_password("").is_err());
        // 按字符数而非字节数计量，多字节口令不应被误判过短
        assert!(hash_web_password("密码密码密码密码").is_ok());
    }

    #[test]
    fn reports_a_malformed_stored_hash() {
        assert!(verify_web_password("correct horse", "not-a-hash").is_err());
    }
}
