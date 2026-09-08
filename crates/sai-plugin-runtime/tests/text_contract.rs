mod common;

use common::{package, runtime, RecordingHost};
use sai_plugin_runtime::{Capabilities, InvocationContext, PluginRuntime};
use serde_json::json;
use std::sync::Arc;

const MARKDOWN_TOOL: &str = r#"
sai.register_tool({
    name = "markdown", description = "Convert HTML",
    parameters = {type = "object", properties = {html = {type = "string"}}, required = {"html"}},
    execute = function(args) return sai.text.html_to_markdown(args.html) end,
})
"#;

/// 【插件测试】【Markdown 契约】保留标题、中文、链接、列表和代码格式。
#[tokio::test]
async fn markdown_preserves_document_structure_and_unicode() {
    let plugin = runtime(MARKDOWN_TOOL);
    let text = plugin
        .call_tool(
            "markdown",
            json!({"html": r#"<h2>输入法</h2><p>参考 <a href="https://example.test/wiki?q=1&amp;lang=zh">官方文档</a>。</p><ul><li>安装 <code>fcitx5</code></li></ul>"#}),
            InvocationContext::default(),
        )
        .await
        .unwrap();
    assert!(text.contains("输入法"));
    assert!(text.contains("[官方文档](https://example.test/wiki?q=1&lang=zh)"));
    assert!(text.contains("fcitx5"));
    assert!(!text.contains("<code>"));
    assert!(!text.contains("<li>"));
}

/// 【插件测试】【转换上限】输入和展开后的输出都遵守字节限制，失败后仍可继续调用。
#[tokio::test]
async fn markdown_limits_input_and_expanded_output() {
    let mut package = package(MARKDOWN_TOOL);
    package.manifest.limits.output_bytes = 1024;
    let plugin = PluginRuntime::load(
        package,
        json!({}),
        Capabilities::default(),
        Arc::new(RecordingHost::default()),
    )
    .unwrap();
    let error = plugin
        .call_tool(
            "markdown",
            json!({"html": "界".repeat(342)}),
            InvocationContext::default(),
        )
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("HTML input exceeds plugin size limit"));

    let html = "_".repeat(700);
    assert!(html2md::parse_html(&html).len() > 1024);
    let error = plugin
        .call_tool(
            "markdown",
            json!({"html": html}),
            InvocationContext::default(),
        )
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("HTML output exceeds plugin size limit"));
    assert_eq!(
        plugin
            .call_tool(
                "markdown",
                json!({"html": "<p>正常</p>"}),
                InvocationContext::default(),
            )
            .await
            .unwrap(),
        "正常"
    );
}

/// 【插件测试】【文本兼容】移动绑定后保留 URL 编码、文本转换和 Unicode 截断语义。
#[tokio::test]
async fn existing_text_helpers_keep_their_contract() {
    let plugin = runtime(
        r#"
        sai.register_tool({name="text",description="Existing text helpers",parameters={type="object"},
            execute=function()
                return {
                    url=sai.text.url_encode("输入 +"),
                    plain=sai.text.html_to_text("<p>中文</p>"),
                    clipped=sai.text.clip("输入法", 2),
                    exact=sai.text.clip("输入法", 3),
                    trimmed=sai.text.trim("　输入法  "),
                }
            end})
    "#,
    );
    let text = plugin
        .call_tool("text", json!({}), InvocationContext::default())
        .await
        .unwrap();
    let result: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(result["url"], "%E8%BE%93%E5%85%A5%20%2B");
    assert!(result["plain"].as_str().unwrap().contains("中文"));
    assert_eq!(result["clipped"], "输入\n...[truncated]");
    assert_eq!(result["exact"], "输入法");
    assert_eq!(result["trimmed"], "输入法");
}
