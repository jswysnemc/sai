//! 模型输出 JSON 的容错解析。
//!
//! 模型流式吐工具参数时，偶尔会在一个完整 JSON 值之后残留一段内容：另一个
//! JSON、一段说明文字、或截断后拼接的残片。严格解析会把它报成
//! "trailing characters" 并让整次工具调用失败，长参数（例如一次写入几百行
//! 文本）尤其容易触发；而入参本身是完好的。
//!
//! 这里给出统一的容错入口 [`first_json_object`]，供工具参数、渐进式调用外壳、
//! 锚点工具以及参数后处理共用，避免各处各写一份宽松解析而行为不一致。
//!
//! 只用于模型输出：磁盘上的配置与状态文件必须严格解析，宽松解析会掩盖真正的
//! 数据损坏。

use anyhow::{bail, Context, Result};
use serde_json::Value;

/// 解析模型输出的 JSON 文本，严格解析失败时退回第一个完整 JSON 对象。
///
/// 参数:
/// - `text`: 模型输出的原始文本，允许前后空白
///
/// 返回:
/// - 解析出的 JSON 值
///
/// 严格解析通过时原样返回，不做类型限制，非对象值交给调用方给出更具体的错误。
///
/// 容错路径只接受对象：工具参数、`invoke_tool` 外壳、`load` 参数在契约上都是
/// 对象。若取到的第一个完整值是标量或数组，这段输出本来就不是一次调用，继续
/// 往下走只会让工具拿着无意义的入参跑起来（例如 `run_command` 缺 `command`）。
/// 同样地，这里也不向前扫描去找第一个 `{`：模型在说明文字里写的示例对象会被
/// 当成真正的参数执行，比直接失败危险得多。这两种情况一律返回错误让模型重试。
pub(crate) fn first_json_object(text: &str) -> Result<Value> {
    let trimmed = text.trim();
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        return Ok(value);
    }
    let mut stream = serde_json::Deserializer::from_str(trimmed).into_iter::<Value>();
    match stream.next() {
        Some(Ok(value)) if value.is_object() => Ok(value),
        Some(Ok(value)) => bail!(
            "expected a single JSON object, found {} followed by trailing content",
            value_kind(&value)
        ),
        Some(Err(error)) => Err(error).context("not valid JSON"),
        None => bail!("not valid JSON: no JSON value found"),
    }
}

/// 返回 JSON 值的类型名称，用于解析失败说明。
///
/// 参数:
/// - `value`: 已解析的 JSON 值
///
/// 返回:
/// - 类型名称
fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 严格有效的 JSON 原样返回，包括非对象值。
    #[test]
    fn strict_json_is_returned_unchanged() {
        assert_eq!(
            first_json_object(r#"{"path":"a.rs"}"#).unwrap(),
            json!({"path": "a.rs"})
        );
        assert_eq!(first_json_object("[1,2]").unwrap(), json!([1, 2]));
        assert_eq!(first_json_object("42").unwrap(), json!(42));
    }

    /// 有效参数后跟另一个完整对象时取第一个，不因为尾随内容让调用失败。
    ///
    /// 长参数（例如几百行的写入内容）最容易触发这种残留。
    #[test]
    fn keeps_the_first_object_when_another_object_follows() {
        assert_eq!(
            first_json_object(r#"{"command":"echo probe-again"}{"command":"rm -rf /"}"#).unwrap(),
            json!({"command": "echo probe-again"})
        );
    }

    /// 有效参数后跟说明文字时取前面的对象。
    #[test]
    fn keeps_the_object_when_prose_follows() {
        assert_eq!(
            first_json_object("{\"path\":\"a\"}\n这里还有一段解释").unwrap(),
            json!({"path": "a"})
        );
        assert_eq!(
            first_json_object(r#"{"files":[{"path":"a.rs"}],"note":"x"} trailing"#).unwrap(),
            json!({"files": [{"path": "a.rs"}], "note": "x"})
        );
    }

    /// 截断、完全非 JSON、空输入都要失败，不能静默吞掉坏输入。
    #[test]
    fn rejects_input_without_a_complete_object() {
        assert!(first_json_object("这不是 JSON").is_err());
        assert!(first_json_object("{broken").is_err());
        assert!(first_json_object(r#"{"path":"/tmp/a"#).is_err());
        assert!(first_json_object("").is_err());
    }

    /// 容错不接受标量或数组：工具参数在契约上是对象，放行等于让工具无参执行。
    #[test]
    fn trailing_content_after_a_non_object_still_fails() {
        let array = first_json_object("[\"a\",\"b\"] trailing").unwrap_err();
        assert!(array.to_string().contains("found array"), "{array}");

        let scalar = first_json_object("123 trailing").unwrap_err();
        assert!(scalar.to_string().contains("found number"), "{scalar}");

        assert!(first_json_object("\"just a string\" trailing").is_err());
        assert!(first_json_object("2{\"path\":\"a\"}").is_err());
    }

    /// 说明文字在前时不做前向扫描：那会把示例对象当成真实参数执行。
    #[test]
    fn does_not_scan_forward_past_leading_prose() {
        let error = first_json_object("可以这样调用：\n{\"path\":\"a.rs\"}").unwrap_err();
        assert!(error.to_string().contains("not valid JSON"), "{error}");
    }

    /// 错误信息要能提示模型重新构造参数。
    #[test]
    fn error_messages_stay_actionable() {
        let malformed = format!("{:#}", first_json_object("这不是 JSON").unwrap_err());
        assert!(malformed.contains("not valid JSON"), "{malformed}");
        assert!(malformed.contains("line 1 column"), "{malformed}");

        let truncated = format!("{:#}", first_json_object(r#"{"path":"/tmp/a"#).unwrap_err());
        assert!(truncated.contains("not valid JSON"), "{truncated}");
        assert!(truncated.contains("line 1 column"), "{truncated}");
    }
}
