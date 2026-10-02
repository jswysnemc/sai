use super::{http::Response, request::Format};
use anyhow::{Context, Result};
use std::{
    sync::{Arc, OnceLock},
    time::Duration,
};

/// 【网页读取】【正文转换】参数为响应、格式与字符上限，返回正文；转换并发有界且不占用异步执行线程。
pub(super) async fn render(response: Response, format: Format, max_chars: usize) -> Result<String> {
    static SLOTS: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
    let slots = SLOTS
        .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(4)))
        .clone();
    tokio::time::timeout(Duration::from_secs(30), async move {
        let permit = slots.acquire_owned().await?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let body = decode(&response);
            let mime = response.content_type.split(';').next().unwrap_or("").trim();
            let is_html = mime.eq_ignore_ascii_case("text/html")
                || mime.eq_ignore_ascii_case("application/xhtml+xml");
            let text = match (is_html, format) {
                (true, Format::Markdown) => super::html::markdown(&body),
                (true, Format::Text) => html2text::from_read(body.as_bytes(), 120),
                _ => body,
            };
            truncate(&text, max_chars)
        })
        .await
        .context("web fetch content conversion failed")
    })
    .await
    .context("web fetch content conversion timed out")?
}

/// 【网页读取】【字符解码】参数为原始响应，返回优先采用 BOM、其次 charset、最后 UTF-8 的文本。
fn decode(response: &Response) -> String {
    let declared = response.content_type.split(';').skip(1).find_map(|part| {
        let (key, value) = part.trim().split_once('=')?;
        key.eq_ignore_ascii_case("charset")
            .then(|| {
                encoding_rs::Encoding::for_label(value.trim().trim_matches(['\'', '"']).as_bytes())
            })
            .flatten()
    });
    let (encoding, offset) = encoding_rs::Encoding::for_bom(&response.bytes)
        .unwrap_or((declared.unwrap_or(encoding_rs::UTF_8), 0));
    encoding.decode(&response.bytes[offset..]).0.into_owned()
}

/// 【网页读取】【字符裁剪】参数为完整正文和字符上限，返回不切断 Unicode 字符的前缀及原接口截断说明。
fn truncate(text: &str, limit: usize) -> String {
    let total = text.chars().count();
    if total <= limit {
        text.to_string()
    } else {
        format!(
            "{}\n\n[content truncated from {total} chars to {limit} chars]",
            text.chars().take(limit).collect::<String>()
        )
    }
}
