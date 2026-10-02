use anyhow::{bail, Result};
use serde_json::Value;

#[derive(Clone, Copy)]
pub(super) enum Format {
    Markdown,
    Text,
    Html,
}

pub(super) struct FetchInput {
    pub url: reqwest::Url,
    pub format: Format,
    pub timeout: u64,
    pub max_chars: usize,
}

impl FetchInput {
    /// 【网页读取】【输入校验】参数为 JSON 参数，返回 URL、格式与沿用原接口范围的限制。
    pub fn parse(args: &Value) -> Result<Self> {
        let value = args["url"].as_str().unwrap_or_default().trim();
        let url =
            reqwest::Url::parse(value).map_err(|_| anyhow::anyhow!("invalid web fetch URL"))?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            bail!("URL must start with http:// or https://");
        }
        let format = match args["format"].as_str().unwrap_or("markdown") {
            "markdown" => Format::Markdown,
            "text" => Format::Text,
            "html" => Format::Html,
            _ => bail!("unsupported web fetch format"),
        };
        Ok(Self {
            url,
            format,
            timeout: args["timeout"].as_u64().unwrap_or(30).min(120),
            max_chars: args["max_chars"]
                .as_u64()
                .unwrap_or(24_000)
                .clamp(1, 80_000) as usize,
        })
    }
}
