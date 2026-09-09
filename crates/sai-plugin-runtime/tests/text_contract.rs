mod common;

use common::{package, runtime, RecordingHost};
use sai_plugin_runtime::{Capabilities, InvocationContext, PluginRuntime};
use serde_json::{json, Value};
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
                    collapsed=sai.text.collapse_whitespace("　输入法 \t 与\n Lua　"),
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
    assert_eq!(result["collapsed"], "输入法 与 Lua");
}

/// 【插件测试】【Unicode 大写】保留非 ASCII 转换，并在调用边界拒绝非文本参数。
#[tokio::test]
async fn uppercase_uses_unicode_rules_and_requires_strings() {
    let plugin = runtime(
        r#"
        sai.register_tool({name="upper",description="Unicode uppercase",parameters={type="object"},
            execute=function(args) return sai.text.upper(args.value) end})
        "#,
    );
    for (value, expected) in [
        ("", ""),
        ("人民币 usd", "人民币 USD"),
        ("straße ﬃ ŉ", "STRASSE FFI ʼN"),
        ("éσςя", "ÉΣΣЯ"),
    ] {
        assert_eq!(
            plugin
                .call_tool(
                    "upper",
                    json!({"value":value}),
                    InvocationContext::default()
                )
                .await
                .unwrap(),
            expected
        );
    }
    for value in [Value::Null, json!(false), json!(42), json!([]), json!({})] {
        let error = plugin
            .call_tool(
                "upper",
                json!({"value":value}),
                InvocationContext::default(),
            )
            .await
            .unwrap_err();
        assert!(format!("{error:#}").contains("upper requires a string"));
    }
}

/// 【插件测试】【大写字节边界】输入和 Unicode 展开均检查字节预算，等于上限时成功。
#[tokio::test]
async fn uppercase_checks_input_and_expanded_output_byte_limits() {
    let mut package = package(
        r#"
        sai.register_tool({name="upper",description="Bounded uppercase",parameters={type="object"},
            execute=function(args)
                local value = sai.text.upper(args.value)
                return {bytes=#value, matches=value==args.expected}
            end})
        "#,
    );
    package.manifest.limits.output_bytes = 1024;
    let plugin = PluginRuntime::load(
        package,
        json!({}),
        Capabilities::default(),
        Arc::new(RecordingHost::default()),
    )
    .unwrap();
    for (value, stage) in [("界".repeat(342), "input"), ("ŉ".repeat(512), "output")] {
        let error = plugin
            .call_tool(
                "upper",
                json!({"value":value}),
                InvocationContext::default(),
            )
            .await
            .unwrap_err();
        assert!(format!("{error:#}").contains(&format!("text {stage} exceeds plugin size limit")));
    }
    for (value, expected) in [
        ("a".repeat(1024), "A".repeat(1024)),
        (
            format!("{}a", "ŉ".repeat(341)),
            format!("{}A", "ʼN".repeat(341)),
        ),
    ] {
        let output = plugin
            .call_tool(
                "upper",
                json!({"value":value,"expected":expected}),
                InvocationContext::default(),
            )
            .await
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&output).unwrap(),
            json!({"bytes":1024,"matches":true})
        );
    }
}

/// 【插件测试】【数值展示】保留整数的 f64 语义、负零与极端有限浮点数的十进制精度。
#[tokio::test]
async fn number_to_string_preserves_f64_decimal_formatting() {
    let plugin = runtime(
        r#"
        sai.register_tool({name="number",description="Decimal numbers",parameters={type="object"},
            execute=function(args) return sai.text.number_to_string(args.value) end})
        "#,
    );
    for (value, expected) in [
        (json!(1), "1".to_string()),
        (json!(9007199254740993_i64), "9007199254740992".to_string()),
        (json!(-0.0), "-0".to_string()),
        (
            json!(7.123456789012345_f64),
            "7.123456789012345".to_string(),
        ),
        (json!(1e-20), "0.00000000000000000001".to_string()),
        (json!(1e20), "100000000000000000000".to_string()),
        (json!(f64::MAX), f64::MAX.to_string()),
        (json!(f64::from_bits(1)), f64::from_bits(1).to_string()),
    ] {
        assert_eq!(
            plugin
                .call_tool(
                    "number",
                    json!({"value":value}),
                    InvocationContext::default()
                )
                .await
                .unwrap(),
            expected
        );
    }
    for value in [
        Value::Null,
        json!(false),
        json!("1.25"),
        json!([]),
        json!({}),
    ] {
        let error = plugin
            .call_tool(
                "number",
                json!({"value":value}),
                InvocationContext::default(),
            )
            .await
            .unwrap_err();
        assert!(format!("{error:#}").contains("number_to_string requires a number"));
    }
}

/// 【插件测试】【非有限数值】拒绝 Lua 计算产生的 NaN 和无穷大，失败后实例仍可继续使用。
#[tokio::test]
async fn number_to_string_rejects_nonfinite_lua_numbers() {
    let plugin = runtime(
        r#"
        sai.register_tool({name="number",description="Finite numbers",parameters={type="object"},
            execute=function(args)
                local values = {nan=0/0, infinity=math.huge, negative=-math.huge, valid=1.5}
                return sai.text.number_to_string(values[args.kind])
            end})
        "#,
    );
    for kind in ["nan", "infinity", "negative"] {
        let error = plugin
            .call_tool("number", json!({"kind":kind}), InvocationContext::default())
            .await
            .unwrap_err();
        assert!(format!("{error:#}").contains("number_to_string requires a finite number"));
    }
    assert_eq!(
        plugin
            .call_tool(
                "number",
                json!({"kind":"valid"}),
                InvocationContext::default()
            )
            .await
            .unwrap(),
        "1.5"
    );
}
