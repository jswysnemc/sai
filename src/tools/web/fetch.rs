use anyhow::{bail, Result};
use serde_json::Value;
use std::time::Duration;

const MAX_RESPONSE_SIZE: usize = 5 * 1024 * 1024;
const DEFAULT_FETCH_MAX_CHARS: usize = 24_000;
const MAX_FETCH_CHARS: usize = 80_000;

/// 【网页读取】【请求执行】读取已知 HTTP(S) 地址并按指定格式返回内容。
///
/// 参数:
/// - `args`: 地址、输出格式、超时和字符上限
///
/// 返回:
/// - 转换并裁剪后的网页内容
pub(super) async fn web_fetch(args: Value) -> Result<String> {
    let url = args
        .get("url")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if !url.starts_with("http://") && !url.starts_with("https://") {
        bail!("URL must start with http:// or https://");
    }
    let format = args
        .get("format")
        .and_then(Value::as_str)
        .unwrap_or("markdown");
    let timeout = args
        .get("timeout")
        .and_then(Value::as_u64)
        .unwrap_or(30)
        .min(120);
    let max_chars = args
        .get("max_chars")
        .and_then(Value::as_u64)
        .map(|value| value.clamp(1, MAX_FETCH_CHARS as u64) as usize)
        .unwrap_or(DEFAULT_FETCH_MAX_CHARS);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout))
        .build()?;
    let accept = match format {
        "text" => "text/plain;q=1.0, text/markdown;q=0.9, text/html;q=0.8, */*;q=0.1",
        "html" => "text/html;q=1.0, application/xhtml+xml;q=0.9, text/plain;q=0.8, */*;q=0.1",
        _ => "text/markdown;q=1.0, text/x-markdown;q=0.9, text/plain;q=0.8, text/html;q=0.7, */*;q=0.1",
    };
    let response = client
        .get(url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36")
        .header("Accept", accept)
        .header("Accept-Language", "en-US,en;q=0.9")
        .send()
        .await?
        .error_for_status()?;
    if response.content_length().unwrap_or(0) > MAX_RESPONSE_SIZE as u64 {
        bail!("response too large (exceeds 5MB limit)");
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    let bytes = response.bytes().await?;
    if bytes.len() > MAX_RESPONSE_SIZE {
        bail!("response too large (exceeds 5MB limit)");
    }
    let content = String::from_utf8_lossy(&bytes).to_string();
    let output = if content_type.contains("text/html") {
        match format {
            "html" => content,
            "text" => html2text::from_read(content.as_bytes(), 120),
            _ => html2md::parse_html(&content),
        }
    } else {
        content
    };
    Ok(clip_fetch_output(&output, max_chars))
}

/// 【网页读取】【内容裁剪】按字符数量裁剪网页内容并附加说明。
///
/// 参数:
/// - `value`: 原始网页内容
/// - `max_chars`: 最大字符数量
///
/// 返回:
/// - 未超限原文或带裁剪说明的内容
fn clip_fetch_output(value: &str, max_chars: usize) -> String {
    let total = value.chars().count();
    if total <= max_chars {
        return value.to_string();
    }
    let clipped = value.chars().take(max_chars).collect::<String>();
    format!("{clipped}\n\n[content truncated from {total} chars to {max_chars} chars]")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clips_fetch_output_with_notice() {
        let output = clip_fetch_output("abcdef", 3);

        assert_eq!(output, "abc\n\n[content truncated from 6 chars to 3 chars]");
    }

    #[test]
    fn keeps_short_fetch_output_unchanged() {
        assert_eq!(clip_fetch_output("abc", 3), "abc");
    }
}
