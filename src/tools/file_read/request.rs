use crate::i18n::text as t;
use crate::tools::fs_path::expand_path;
use anyhow::{bail, Result};
use serde_json::Value;
use std::path::PathBuf;

/// 单次 read_file 调用的解析结果。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ReadRequest {
    /// 模型传入的原始路径，错误提示原样回显
    pub(super) raw_path: String,
    /// 展开后的绝对路径
    pub(super) path: PathBuf,
    /// 1 起始的起始行或目录项
    pub(super) offset: usize,
    /// 最多读取的行数或目录项；缺省表示读完整文件
    pub(super) limit: Option<usize>,
    /// PDF 页码范围原文
    pub(super) pages: Option<String>,
}

impl ReadRequest {
    /// 从工具参数解析读取请求。
    ///
    /// offset 与 limit 同时接受整数和纯数字字符串；offset 小于 1 时按第 1 行处理。
    ///
    /// 参数:
    /// - `args`: 工具参数 JSON
    ///
    /// 返回:
    /// - 读取请求；缺少路径或 limit 非正数时报错
    pub(super) fn from_value(args: &Value) -> Result<Self> {
        let raw_path = args
            .get("path")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or_default()
            .to_string();
        if raw_path.is_empty() {
            bail!("{}: path", t("required argument missing", "缺少必需参数"))
        }
        let offset = integer_field(args, "offset")?.unwrap_or(1).max(1) as usize;
        let limit = match integer_field(args, "limit")? {
            Some(value) if value <= 0 => bail!("limit must be a positive integer"),
            Some(value) => Some(value as usize),
            None => None,
        };
        let pages = args
            .get("pages")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        Ok(Self {
            path: expand_path(&raw_path),
            raw_path,
            offset,
            limit,
            pages,
        })
    }

    /// 返回小写扩展名，没有扩展名时为空串。
    ///
    /// 返回:
    /// - 小写扩展名
    pub(super) fn extension(&self) -> String {
        self.path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
    }
}

/// 读取可选整数字段，兼容数字字符串。
///
/// 参数:
/// - `args`: 工具参数
/// - `key`: 字段名
///
/// 返回:
/// - 字段缺失或为 null 时为 None；无法解析为整数时报错
fn integer_field(args: &Value, key: &str) -> Result<Option<i64>> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(number)) => number
            .as_i64()
            .or_else(|| {
                number
                    .as_f64()
                    .filter(|value| value.fract() == 0.0)
                    .map(|value| value as i64)
            })
            .map(Some)
            .ok_or_else(|| anyhow::anyhow!("{key} must be an integer")),
        Some(Value::String(text)) if text.trim().is_empty() => Ok(None),
        Some(Value::String(text)) => text
            .trim()
            .parse::<i64>()
            .map(Some)
            .map_err(|_| anyhow::anyhow!("{key} must be an integer")),
        Some(_) => bail!("{key} must be an integer"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 缺省 offset 从第 1 行开始，limit 缺省表示整文件。
    #[test]
    fn defaults_to_whole_file_from_first_line() {
        let request = ReadRequest::from_value(&json!({"path": "/tmp/a.txt"})).unwrap();
        assert_eq!(request.offset, 1);
        assert_eq!(request.limit, None);
        assert_eq!(request.pages, None);
    }

    /// 数字字符串与 0 起始 offset 都按行号语义处理。
    #[test]
    fn accepts_numeric_strings_and_zero_offset() {
        let request =
            ReadRequest::from_value(&json!({"path": "/tmp/a.txt", "offset": "0", "limit": "5"}))
                .unwrap();
        assert_eq!(request.offset, 1);
        assert_eq!(request.limit, Some(5));
    }

    /// 非正数 limit 与缺失路径直接报错。
    #[test]
    fn rejects_invalid_limit_and_missing_path() {
        assert!(ReadRequest::from_value(&json!({"path": "/tmp/a", "limit": 0})).is_err());
        assert!(ReadRequest::from_value(&json!({"offset": 3})).is_err());
    }
}
