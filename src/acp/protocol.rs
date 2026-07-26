use serde::{Deserialize, Serialize};
use serde_json::Value;

/// sai 支持的 ACP 协议版本。
///
/// 规范文档把 `protocolVersion` 写成字符串，但实测 claude-code-acp 0.16 与
/// codex-acp 1.1 都要求数字，传字符串直接回 `-32602 Invalid params`。
pub(crate) const PROTOCOL_VERSION: u32 = 1;

/// JSON-RPC 请求标识。
///
/// 规范允许数字或字符串，实现方各用各的，因此原样保留以便回包时匹配。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum RequestId {
    Number(i64),
    Text(String),
}

/// 从对端读到的一条消息。
///
/// ACP 是双向协议：agent 既会回应 sai 的请求，也会主动请求 sai 提供文件、
/// 终端与权限决定，还会推送 `session/update` 通知。
#[derive(Debug, Clone)]
pub(crate) enum Incoming {
    /// 对我方请求的响应
    Response {
        id: RequestId,
        result: Result<Value, JsonRpcError>,
    },
    /// 对端发起的请求，需要我方回包
    Request {
        id: RequestId,
        method: String,
        params: Value,
    },
    /// 对端发来的通知，无需回包
    Notification { method: String, params: Value },
}

/// JSON-RPC 错误对象。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct JsonRpcError {
    pub(crate) code: i64,
    pub(crate) message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) data: Option<Value>,
}

impl std::fmt::Display for JsonRpcError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} (code {})", self.message, self.code)
    }
}

/// 解析对端发来的一行 JSON-RPC 消息。
///
/// 按 JSON-RPC 2.0 的判定顺序：带 `method` 的是请求或通知（有无 `id` 区分），
/// 其余按响应处理。
///
/// 参数:
/// - `line`: 一行 JSON 文本
///
/// 返回:
/// - 解析结果；不是合法消息时返回 None
pub(crate) fn parse_incoming(line: &str) -> Option<Incoming> {
    let value = serde_json::from_str::<Value>(line).ok()?;
    let object = value.as_object()?;
    let id = object.get("id").and_then(parse_request_id);
    if let Some(method) = object.get("method").and_then(Value::as_str) {
        let params = object.get("params").cloned().unwrap_or(Value::Null);
        return Some(match id {
            Some(id) => Incoming::Request {
                id,
                method: method.to_string(),
                params,
            },
            None => Incoming::Notification {
                method: method.to_string(),
                params,
            },
        });
    }
    let id = id?;
    if let Some(error) = object.get("error") {
        let error = serde_json::from_value::<JsonRpcError>(error.clone()).ok()?;
        return Some(Incoming::Response {
            id,
            result: Err(error),
        });
    }
    Some(Incoming::Response {
        id,
        result: Ok(object.get("result").cloned().unwrap_or(Value::Null)),
    })
}

/// 解析 JSON-RPC 标识。
///
/// 参数:
/// - `value`: id 字段
///
/// 返回:
/// - 数字或字符串标识
fn parse_request_id(value: &Value) -> Option<RequestId> {
    match value {
        Value::Number(number) => number.as_i64().map(RequestId::Number),
        Value::String(text) => Some(RequestId::Text(text.clone())),
        _ => None,
    }
}

/// 组装一条 JSON-RPC 请求。
///
/// 参数:
/// - `id`: 请求标识
/// - `method`: 方法名
/// - `params`: 参数
///
/// 返回:
/// - 待发送的 JSON 值
pub(crate) fn request(id: i64, method: &str, params: Value) -> Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    })
}

/// 组装一条 JSON-RPC 通知。
///
/// 参数:
/// - `method`: 方法名
/// - `params`: 参数
///
/// 返回:
/// - 待发送的 JSON 值
pub(crate) fn notification(method: &str, params: Value) -> Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
    })
}

/// 组装对端请求的成功响应。
///
/// 参数:
/// - `id`: 对端请求标识
/// - `result`: 响应内容
///
/// 返回:
/// - 待发送的 JSON 值
pub(crate) fn response(id: &RequestId, result: Value) -> Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    })
}

/// 组装对端请求的错误响应。
///
/// 参数:
/// - `id`: 对端请求标识
/// - `code`: 错误码
/// - `message`: 错误描述
///
/// 返回:
/// - 待发送的 JSON 值
pub(crate) fn error_response(id: &RequestId, code: i64, message: &str) -> Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    })
}

/// 方法未实现的标准错误码。
pub(crate) const METHOD_NOT_FOUND: i64 = -32601;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_success_response() {
        let parsed = parse_incoming(r#"{"jsonrpc":"2.0","id":1,"result":{"ok":true}}"#).unwrap();
        match parsed {
            Incoming::Response { id, result } => {
                assert_eq!(id, RequestId::Number(1));
                assert_eq!(result.unwrap()["ok"], true);
            }
            other => panic!("expected a response, got {other:?}"),
        }
    }

    #[test]
    fn parses_error_response() {
        let parsed =
            parse_incoming(r#"{"jsonrpc":"2.0","id":"a","error":{"code":-32602,"message":"bad"}}"#)
                .unwrap();
        match parsed {
            Incoming::Response { id, result } => {
                assert_eq!(id, RequestId::Text("a".to_string()));
                assert_eq!(result.unwrap_err().code, -32602);
            }
            other => panic!("expected a response, got {other:?}"),
        }
    }

    /// 带 id 的 method 是请求，需要回包；不带 id 的是通知。
    #[test]
    fn distinguishes_requests_from_notifications() {
        let request = parse_incoming(
            r#"{"jsonrpc":"2.0","id":7,"method":"session/request_permission","params":{}}"#,
        )
        .unwrap();
        assert!(matches!(request, Incoming::Request { .. }));

        let notification =
            parse_incoming(r#"{"jsonrpc":"2.0","method":"session/update","params":{}}"#).unwrap();
        assert!(matches!(notification, Incoming::Notification { .. }));
    }

    #[test]
    fn rejects_non_json_lines() {
        assert!(parse_incoming("not json").is_none());
        assert!(parse_incoming("").is_none());
    }

    /// 实测两个适配器都要求数字版本号，字符串会被拒绝。
    #[test]
    fn initialize_uses_numeric_protocol_version() {
        let value = request(
            1,
            "initialize",
            serde_json::json!({ "protocolVersion": PROTOCOL_VERSION }),
        );
        assert!(value["params"]["protocolVersion"].is_number());
    }
}
