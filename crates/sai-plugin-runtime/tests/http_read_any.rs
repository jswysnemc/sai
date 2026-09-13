mod common;

use common::{package, RecordingHost};
use sai_plugin_runtime::{Capabilities, InvocationContext, PluginRuntime};
use serde_json::json;
use std::sync::Arc;

/// 【网络授权测试】【任意只读】明确授权只扩大 GET 和 HEAD 的来源范围
/// @returns 无；写入、其他协议、内嵌凭据和精确来源入口仍然拒绝
#[test]
fn arbitrary_read_grant_preserves_method_and_url_boundaries() {
    let capabilities: Capabilities = serde_json::from_value(json!({"http_read_any":true})).unwrap();
    capabilities.validate().unwrap();
    for url in [
        "https://example.test/a",
        "http://127.0.0.1:1234/a",
        "http://[::1]/a",
    ] {
        for method in ["GET", "HEAD"] {
            assert!(capabilities.authorize_request(method, url, false).is_ok());
        }
        assert!(capabilities.authorize_url(url).is_err());
        for method in ["POST", "PUT", "PATCH", "DELETE", "OPTIONS", "TRACE"] {
            for writable in [true, false] {
                assert!(capabilities
                    .authorize_request(method, url, writable)
                    .is_err());
            }
        }
    }
    for url in ["file:///etc/passwd", "https://name:secret@example.test/a"] {
        let error = capabilities
            .authorize_request("GET", url, false)
            .unwrap_err();
        assert!(!format!("{error:#}").contains("secret"));
    }
}

/// 【网络授权测试】【声明交集】任意来源只读权限需要清单与用户授权同时存在
/// @returns 无；缺省和任一撤销方向都不能访问未声明来源
#[tokio::test]
async fn arbitrary_reads_require_both_declaration_and_grant() {
    for (declared, granted) in [(true, true), (true, false), (false, true), (false, false)] {
        let mut package = package(
            r#"
            sai.register_tool({name='read',description='Read HTTP',parameters={type='object'},
                execute=function() return sai.http.request({url='http://127.0.0.1:1234/a'}).text end})
        "#,
        );
        package.manifest.capabilities =
            serde_json::from_value(json!({"http_read_any":declared})).unwrap();
        let grants = serde_json::from_value(json!({"http_read_any":granted})).unwrap();
        let host = Arc::new(RecordingHost::default());
        let plugin = PluginRuntime::load(package, json!({}), grants, host.clone()).unwrap();
        let result = plugin
            .call_tool("read", json!({}), InvocationContext::default())
            .await;
        assert_eq!(result.is_ok(), declared && granted);
        assert_eq!(
            host.requests.lock().unwrap().len(),
            usize::from(declared && granted)
        );
    }
}

/// 【网络授权测试】【精确写入】新增只读来源范围不能改变原有精确来源和 POST 查询授权
/// @returns 无；只读 POST 与显式写入上下文仍按原规则执行
#[test]
fn arbitrary_reads_compose_with_existing_exact_write_grants() {
    let capabilities: Capabilities = serde_json::from_value(json!({
        "http_read_any":true,"http":["https://service.test"],
        "http_read_only_post":["https://service.test/search"]
    }))
    .unwrap();
    assert!(capabilities
        .authorize_request("POST", "https://service.test/search", false)
        .is_ok());
    assert!(capabilities
        .authorize_request("POST", "https://service.test/delete", false)
        .is_err());
    assert!(capabilities
        .authorize_request("DELETE", "https://service.test/item", true)
        .is_ok());
    assert!(!capabilities.is_subset(&Capabilities::default()));
    let reduced = capabilities.intersection(&Capabilities::default());
    assert!(reduced
        .authorize_request("GET", "http://localhost/", false)
        .is_err());
}
