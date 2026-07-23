use anyhow::{bail, Context, Result};
use serde_json::Value;

/// 判断 JSON-RPC 响应是否匹配请求标识。
///
/// 参数:
/// - `value`: JSON-RPC 响应
/// - `id`: 期望的请求标识
///
/// 返回:
/// - 整数或有符号整数标识匹配时返回 `true`
pub(super) fn matches_id(value: &Value, id: u64) -> bool {
    value.get("id").and_then(|value| value.as_u64()) == Some(id)
        || value.get("id").and_then(|value| value.as_i64()) == Some(id as i64)
}

/// 解析 HTTP 或 SSE 承载的 JSON-RPC 响应。
///
/// 参数:
/// - `body`: 响应正文
/// - `content_type`: HTTP 内容类型
/// - `id`: 期望的请求标识
///
/// 返回:
/// - 匹配响应的 `result` 值或解析错误
pub(super) fn parse_rpc_body(body: &str, content_type: &str, id: u64) -> Result<Value> {
    if content_type.contains("text/event-stream") || body.contains("event:") {
        for chunk in body.split("\n\n") {
            for line in chunk.lines() {
                let line = line.trim();
                if let Some(data) = line.strip_prefix("data:") {
                    let data = data.trim();
                    if data.is_empty() || data == "[DONE]" {
                        continue;
                    }
                    if let Ok(value) = serde_json::from_str::<Value>(data) {
                        if matches_id(&value, id) {
                            if let Some(error) = value.get("error") {
                                bail!("mcp error: {error}");
                            }
                            return Ok(value.get("result").cloned().unwrap_or(Value::Null));
                        }
                    }
                }
            }
        }
        bail!("sse response missing matching rpc id");
    }
    let value: Value = serde_json::from_str(body).context("invalid mcp json response")?;
    if matches_id(&value, id) {
        if let Some(error) = value.get("error") {
            bail!("mcp error: {error}");
        }
        return Ok(value.get("result").cloned().unwrap_or(Value::Null));
    }
    // 部分 HTTP 实现直接返回 result 对象
    Ok(value)
}

/// 从 SSE 响应中提取消息端点。
///
/// 参数:
/// - `body`: SSE 响应正文
/// - `base_url`: SSE 连接地址
///
/// 返回:
/// - 解析后的绝对消息端点
pub(super) fn parse_sse_endpoint(body: &str, base_url: &str) -> Option<String> {
    let mut event = String::new();
    let mut data = String::new();
    for line in body.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            if event == "endpoint" && !data.is_empty() {
                return Some(resolve_url(base_url, data.trim()));
            }
            event.clear();
            data.clear();
            continue;
        }
        if let Some(value) = line.strip_prefix("event:") {
            event = value.trim().to_string();
        } else if let Some(value) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(value.trim());
        }
    }
    if event == "endpoint" && !data.is_empty() {
        return Some(resolve_url(base_url, data.trim()));
    }
    None
}

/// 将相对消息端点解析为绝对 URL。
///
/// 参数:
/// - `base`: 基础 URL
/// - `maybe_relative`: 绝对或相对端点
///
/// 返回:
/// - 解析后的 URL，解析失败时返回原值
fn resolve_url(base: &str, maybe_relative: &str) -> String {
    if maybe_relative.starts_with("http://") || maybe_relative.starts_with("https://") {
        return maybe_relative.to_string();
    }
    if let Ok(base) = reqwest::Url::parse(base) {
        if let Ok(joined) = base.join(maybe_relative) {
            return joined.to_string();
        }
    }
    maybe_relative.to_string()
}
