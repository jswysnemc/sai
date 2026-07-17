use anyhow::{bail, Result};
use serde::{Deserialize, Deserializer};

/// 解析模型上下文长度。
///
/// 参数:
/// - `value`: 表单或配置中的上下文长度，可使用纯数字、k 或 m 单位
///
/// 返回:
/// - 大于 0 时返回 token 数，空值或 0 返回空
pub fn parse_context_chars(value: &str) -> Result<Option<usize>> {
    let value = value.trim().replace('_', "").to_ascii_lowercase();
    if value.is_empty() {
        return Ok(None);
    }

    let (number, multiplier) = split_context_unit(&value)?;
    let tokens = if number.contains('.') {
        parse_decimal_context_chars(number, multiplier)?
    } else {
        parse_integer_context_chars(number, multiplier)?
    };

    if tokens == 0 {
        Ok(None)
    } else {
        Ok(Some(tokens))
    }
}

/// 反序列化可带单位的上下文长度。
///
/// 参数:
/// - `deserializer`: Serde 反序列化器
///
/// 返回:
/// - 上下文长度，空值或 0 返回空
pub fn deserialize_optional_context_chars<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<usize>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<ContextCharsRepr>::deserialize(deserializer)?;
    match value {
        Some(ContextCharsRepr::Number(value)) => {
            if value == 0 {
                Ok(None)
            } else {
                Ok(Some(value))
            }
        }
        Some(ContextCharsRepr::String(value)) => {
            parse_context_chars(&value).map_err(serde::de::Error::custom)
        }
        None => Ok(None),
    }
}

/// 拆分上下文长度单位。
///
/// 参数:
/// - `value`: 已去除空白和下划线的输入
///
/// 返回:
/// - 数字部分和单位倍数
fn split_context_unit(value: &str) -> Result<(&str, usize)> {
    let (number, multiplier) = if let Some(number) = value.strip_suffix('k') {
        (number, 1_000usize)
    } else if let Some(number) = value.strip_suffix('m') {
        (number, 1_000_000usize)
    } else {
        (value, 1usize)
    };

    if number.trim().is_empty() {
        bail!("context length cannot be empty before unit");
    }
    Ok((number, multiplier))
}

/// 解析整数上下文长度。
///
/// 参数:
/// - `number`: 数字部分
/// - `multiplier`: 单位倍数
///
/// 返回:
/// - 换算后的 token 数
fn parse_integer_context_chars(number: &str, multiplier: usize) -> Result<usize> {
    let number = number
        .parse::<usize>()
        .map_err(|_| anyhow::anyhow!("invalid context length: {number}"))?;
    number
        .checked_mul(multiplier)
        .ok_or_else(|| anyhow::anyhow!("context length is too large"))
}

/// 解析小数上下文长度。
///
/// 参数:
/// - `number`: 小数数字部分
/// - `multiplier`: 单位倍数
///
/// 返回:
/// - 换算后的 token 数
fn parse_decimal_context_chars(number: &str, multiplier: usize) -> Result<usize> {
    let number = number
        .parse::<f64>()
        .map_err(|_| anyhow::anyhow!("invalid context length: {number}"))?;
    if !number.is_finite() || number < 0.0 {
        bail!("invalid context length: {number}");
    }
    let tokens = (number * multiplier as f64).round();
    if tokens > usize::MAX as f64 {
        bail!("context length is too large");
    }
    Ok(tokens as usize)
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ContextCharsRepr {
    Number(usize),
    String(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Holder {
        #[serde(default, deserialize_with = "deserialize_optional_context_chars")]
        context_chars: Option<usize>,
    }

    #[test]
    fn parses_context_units() {
        assert_eq!(parse_context_chars("128k").unwrap(), Some(128_000));
        assert_eq!(parse_context_chars("1m").unwrap(), Some(1_000_000));
        assert_eq!(parse_context_chars("200000").unwrap(), Some(200_000));
        assert_eq!(parse_context_chars("1.5m").unwrap(), Some(1_500_000));
    }

    #[test]
    fn empty_or_zero_context_is_unset() {
        assert_eq!(parse_context_chars("").unwrap(), None);
        assert_eq!(parse_context_chars("0").unwrap(), None);
        assert_eq!(parse_context_chars("0k").unwrap(), None);
    }

    #[test]
    fn rejects_invalid_context_units() {
        assert!(parse_context_chars("abc").is_err());
        assert!(parse_context_chars("-1").is_err());
        assert!(parse_context_chars("1g").is_err());
    }

    #[test]
    fn deserializes_string_context_chars() {
        let holder: Holder = serde_json::from_str(r#"{"context_chars":"64k"}"#).unwrap();

        assert_eq!(holder.context_chars, Some(64_000));
    }
}
