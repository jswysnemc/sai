mod common;

use common::{package, RecordingHost};
use sai_plugin_runtime::{Capabilities, InvocationContext, PluginPackage, PluginRuntime};
use serde_json::json;
use std::sync::Arc;

/// 【插件测试】【查询包】构造只读 POST 查询工具，参数决定请求方法与地址。
/// @returns 包含精确来源和查询端点声明的源码快照
fn query_package() -> PluginPackage {
    let mut package = package(
        r#"
        sai.register_tool({
            name="query", description="Read an explicitly authorized query endpoint",
            parameters={type="object"},
            execute=function(args)
                return sai.http.request({
                    url=args.url or "https://service.test/search?q=value",
                    method=args.method or "POST", body="query",
                }).text
            end,
        })
        "#,
    );
    package.manifest.capabilities = serde_json::from_value(json!({
        "http": ["https://service.test"],
        "http_read_only_post": ["https://service.test/search"]
    }))
    .unwrap();
    package
}

/// 【插件测试】【查询授权】只读回调可访问同时声明和授权的 POST 查询端点。
#[tokio::test]
async fn read_only_post_queries_require_both_declaration_and_grant() {
    let package = query_package();
    let host = Arc::new(RecordingHost::default());
    let granted = package.manifest.capabilities.clone();
    let plugin = PluginRuntime::load(package, json!({}), granted, host.clone()).unwrap();
    let output = plugin
        .call_tool("query", json!({}), InvocationContext::default())
        .await
        .unwrap();
    assert_eq!(output, r#"{"answer":42}"#);
    assert_eq!(host.requests.lock().unwrap().len(), 1);

    let package = query_package();
    let granted: Capabilities = serde_json::from_value(json!({
        "http": ["https://service.test"]
    }))
    .unwrap();
    let host = Arc::new(RecordingHost::default());
    let plugin = PluginRuntime::load(package, json!({}), granted, host.clone()).unwrap();
    assert!(plugin
        .call_tool("query", json!({}), InvocationContext::default())
        .await
        .is_err());
    assert!(host.requests.lock().unwrap().is_empty());

    let mut package = query_package();
    let granted = package.manifest.capabilities.clone();
    package.manifest.capabilities.http_read_only_post.clear();
    let host = Arc::new(RecordingHost::default());
    let plugin = PluginRuntime::load(package, json!({}), granted, host.clone()).unwrap();
    assert!(plugin
        .call_tool("query", json!({}), InvocationContext::default())
        .await
        .is_err());
    assert!(host.requests.lock().unwrap().is_empty());
}

/// 【插件测试】【方法隔离】查询授权不能变成任意路径、子路径、来源或写入方法的授权。
#[tokio::test]
async fn query_grants_do_not_authorize_other_paths_or_writing_methods() {
    let package = query_package();
    let granted = package.manifest.capabilities.clone();
    let host = Arc::new(RecordingHost::default());
    let plugin = PluginRuntime::load(package, json!({}), granted, host.clone()).unwrap();
    for args in [
        json!({"url":"https://service.test/delete"}),
        json!({"url":"https://service.test/search/child"}),
        json!({"url":"https://service.test/search/../delete"}),
        json!({"url":"https://service.test/search/%2e%2e/delete"}),
        json!({"url":"https://other.test/search"}),
        json!({"method":"PUT"}),
        json!({"method":"PATCH"}),
        json!({"method":"DELETE"}),
    ] {
        assert!(plugin
            .call_tool("query", args, InvocationContext::default())
            .await
            .is_err());
    }
    assert!(host.requests.lock().unwrap().is_empty());
}

/// 【插件测试】【端点声明】只接受明确来源下的规范路径，不允许通过查询参数暴露凭据。
#[test]
fn read_only_post_declarations_require_normalized_endpoints() {
    for endpoint in [
        "https://service.test/search?key=private",
        "https://service.test/search#fragment",
        "https://service.test/search/../delete",
        "https://name:private@service.test/search",
        "https://other.test/search",
    ] {
        let grants: Capabilities = serde_json::from_value(json!({
            "http":["https://service.test"], "http_read_only_post":[endpoint],
        }))
        .unwrap();
        let error = grants.validate().unwrap_err();
        assert!(!format!("{error:#}").contains("private"));
    }
}
