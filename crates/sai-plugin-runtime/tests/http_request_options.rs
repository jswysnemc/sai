mod common;

use common::{package, RecordingHost};
use sai_plugin_runtime::{InvocationContext, PluginRuntime};
use serde_json::json;
use std::sync::Arc;

/// 【网络选项测试】【旧默认值】新选项不改变普通 HTTP 的五次重定向和错误正文读取
/// @returns 无；显式零次、十次和不读取错误正文均原样交给宿主
#[tokio::test]
async fn redirect_and_error_body_options_are_explicit_and_bounded() {
    let mut package = package(
        r#"
        sai.register_tool({name='read',description='Read response',parameters={type='object'},execute=function(args)
            return sai.http.request({url='https://fixture.test/',max_redirects=args.redirects,read_error_body=args.read_error_body}).status
        end})
    "#,
    );
    package
        .manifest
        .capabilities
        .http
        .insert("https://fixture.test".into());
    let grants = package.manifest.capabilities.clone();
    let host = Arc::new(RecordingHost::default());
    let plugin = PluginRuntime::load(package, json!({}), grants, host.clone()).unwrap();
    for args in [
        json!({}),
        json!({"redirects":0,"read_error_body":false}),
        json!({"redirects":10}),
    ] {
        plugin
            .call_tool("read", args, InvocationContext::default())
            .await
            .unwrap();
    }
    let requests = host.requests.lock().unwrap();
    let values = requests
        .iter()
        .map(|request| serde_json::to_value(request).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        values
            .iter()
            .map(|request| request["max_redirects"].clone())
            .collect::<Vec<_>>(),
        vec![json!(5), json!(0), json!(10)]
    );
    assert_eq!(
        values
            .iter()
            .map(|request| request["read_error_body"].clone())
            .collect::<Vec<_>>(),
        vec![json!(true), json!(false), json!(true)]
    );
    drop(requests);
    for redirects in [json!(11), json!(-1), json!(1.5), json!("3")] {
        assert!(plugin
            .call_tool(
                "read",
                json!({"redirects":redirects}),
                InvocationContext::default()
            )
            .await
            .is_err());
    }
    for read_error_body in [json!(1), json!("false"), json!({}), json!([])] {
        assert!(plugin
            .call_tool(
                "read",
                json!({"read_error_body":read_error_body}),
                InvocationContext::default()
            )
            .await
            .is_err());
    }
    assert_eq!(host.requests.lock().unwrap().len(), 3);
}
