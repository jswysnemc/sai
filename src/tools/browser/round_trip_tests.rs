//! 可选真实浏览器测试，验证统一工具分派以及图片附件不会在合并时丢失。

use crate::tools::ToolRegistry;
use axum::{response::Html, routing::get, Router};
use serde_json::json;

/// 【浏览器工具测试】【真实调用】设置 SAI_BROWSER_TOOL_E2E=1 后通过注册表操作本机测试页面。
#[tokio::test]
async fn unified_browser_round_trip_when_enabled() {
    if std::env::var("SAI_BROWSER_TOOL_E2E").ok().as_deref() != Some("1") {
        return;
    }
    let html = r#"<title>Browser fixture</title><input aria-label="Query">
        <button onclick="document.title=document.querySelector('input').value">Apply</button>"#;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().route("/", get(move || async move { Html(html) })),
        )
        .await
        .unwrap();
    });
    let mut registry = ToolRegistry::new();
    super::register(&mut registry);
    // 1. 【浏览器工具测试】【导航与快照】统一入口打开页面，快照为后续交互提供引用
    call(
        &registry,
        json!({"action":"navigate","url":format!("http://{address}/")}),
    )
    .await;
    let snapshot = call(&registry, json!({"action":"snapshot"})).await;
    let input = reference(&snapshot, "textbox \"Query\"");
    let button = reference(&snapshot, "button \"Apply\"");
    call(
        &registry,
        json!({"action":"type","ref":input,"text":"统一浏览器入口"}),
    )
    .await;
    call(&registry, json!({"action":"click","ref":button})).await;
    let title = call(
        &registry,
        json!({"action":"evaluate","expression":"document.title"}),
    )
    .await;
    assert_eq!(title, "统一浏览器入口");
    call(&registry, json!({"action":"wait","selector":"input"})).await;
    call(&registry, json!({"action":"press_key","key":"Tab"})).await;
    call(
        &registry,
        json!({"action":"scroll","direction":"down","amount":100}),
    )
    .await;

    // 2. 【浏览器工具测试】【截图附件】模型调用路径必须保留图片结果
    let (sender, _receiver) = tokio::sync::mpsc::unbounded_channel();
    let screenshot = registry
        .call_with_progress("browser", r#"{"action":"screenshot"}"#, sender)
        .await
        .unwrap();
    assert!(!screenshot.model_attachments.is_empty());
    assert!(screenshot.content.contains("browser screenshot"));

    // 3. 【浏览器工具测试】【标签管理】新建和关闭仍作用于同一个共享浏览器
    call(&registry, json!({"action":"new_tab"})).await;
    let tabs = call(&registry, json!({"action":"tabs"})).await;
    assert!(tabs.contains("about:blank"));
    call(&registry, json!({"action":"close_tab"})).await;
    let tabs = call(&registry, json!({"action":"tabs"})).await;
    assert!(!tabs.contains("about:blank"));
    crate::browser::shutdown().await;
    server.abort();
}

/// 【浏览器工具测试】【调用辅助】通过真实工具注册表执行动作。
/// @param registry 为注册表；args 为动作参数
/// @returns 工具文本结果
async fn call(registry: &ToolRegistry, args: serde_json::Value) -> String {
    registry
        .call("browser", &args.to_string())
        .await
        .unwrap_or_else(|error| panic!("{args}: {error:#}"))
}

/// 【浏览器工具测试】【引用提取】从快照指定元素行提取引用。
/// @param snapshot 为快照文本；label 为元素角色和名称
/// @returns 元素引用
fn reference(snapshot: &str, label: &str) -> String {
    let line = snapshot.lines().find(|line| line.contains(label)).unwrap();
    let start = line.find("[ref=").unwrap() + 5;
    let end = line[start..].find(']').unwrap() + start;
    line[start..end].to_string()
}
