use crate::i18n::text as t;
use anyhow::{bail, Result};
use serde_json::Value;

pub(crate) const NO_TIMEOUT_SECONDS: u64 = 0;

/// 从工具参数中解析后台任务超时时间。
///
/// 参数:
/// - `args`: 工具参数
/// - `default_seconds`: 配置中的默认超时时间
///
/// 返回:
/// - 后台任务超时时间，0 表示不自动超时
pub(crate) fn timeout_seconds_from_args(args: &Value, default_seconds: u64) -> u64 {
    args.get("timeout_seconds")
        .and_then(Value::as_u64)
        .unwrap_or(default_seconds)
}

/// 从 CLI 参数中解析后台任务超时时间。
///
/// 参数:
/// - `timeout_seconds`: `--timeout-seconds` 参数
/// - `no_timeout`: 是否传入 `--no-timeout`
///
/// 返回:
/// - 需要写入工具参数的超时时间，None 表示使用配置默认值
pub(crate) fn timeout_seconds_from_cli(
    timeout_seconds: Option<u64>,
    no_timeout: bool,
) -> Result<Option<u64>> {
    if no_timeout && timeout_seconds.is_some() {
        bail!(
            "{}",
            t(
                "--no-timeout cannot be used with --timeout-seconds",
                "--no-timeout 不能与 --timeout-seconds 同时使用"
            )
        );
    }
    Ok(if no_timeout {
        Some(NO_TIMEOUT_SECONDS)
    } else {
        timeout_seconds
    })
}

/// 判断后台任务是否不自动超时。
///
/// 参数:
/// - `timeout_seconds`: 后台任务超时时间
///
/// 返回:
/// - 是否为无限时间后台任务
pub(crate) fn is_unlimited(timeout_seconds: u64) -> bool {
    timeout_seconds == NO_TIMEOUT_SECONDS
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_zero_timeout_as_unlimited() {
        let args = json!({"timeout_seconds": 0});

        assert_eq!(timeout_seconds_from_args(&args, 30), 0);
        assert!(is_unlimited(timeout_seconds_from_args(&args, 30)));
    }

    #[test]
    fn uses_config_default_when_timeout_is_absent() {
        let args = json!({});

        assert_eq!(timeout_seconds_from_args(&args, 30), 30);
    }

    #[test]
    fn cli_no_timeout_maps_to_zero() {
        assert_eq!(timeout_seconds_from_cli(None, true).unwrap(), Some(0));
        assert_eq!(timeout_seconds_from_cli(Some(9), false).unwrap(), Some(9));
        assert!(timeout_seconds_from_cli(Some(9), true).is_err());
    }
}
