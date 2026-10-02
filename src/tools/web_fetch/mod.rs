mod http;
mod html;
mod output;
mod request;
#[cfg(test)]
mod tests;

use super::{ToolRegistry, ToolSpec};
use anyhow::Result;
use serde_json::{json, Value};

/// 【网页读取】【原生注册】参数为工具注册表，无返回值；独立于 Lua 插件和搜索供应商注册只读工具。
pub(super) fn register(registry: &mut ToolRegistry) {
    registry.register(ToolSpec::new(
        "web_fetch",
        "Fetch a URL and return markdown, text, or html. Prefer this for opening a known URL. Does not search the web.",
        json!({
            "type":"object", "properties": {
                "url":{"type":"string","description":"Fully-qualified http or https URL."},
                "format":{"type":"string","enum":["markdown","text","html"],"description":"Output format. Defaults to markdown."},
                "timeout":{"type":"integer","description":"Timeout seconds, max 120."},
                "max_chars":{"type":"integer","description":"Maximum characters to return. Defaults to 24000, max 80000."}
            }, "required":["url"], "additionalProperties":false
        }),
        fetch,
    ));
}

/// 【网页读取】【执行入口】参数为公开工具参数，返回转换后的网页正文或不含地址参数的错误。
async fn fetch(args: Value) -> Result<String> {
    let input = request::FetchInput::parse(&args)?;
    let response = http::fetch(&input).await?;
    output::render(response, input.format, input.max_chars).await
}
