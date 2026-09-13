use super::web_fetch_http_support::{response, server};
use crate::plugins::host::SaiPluginHost;
use sai_plugin_runtime::host::{HttpRequest, PluginHost};
use sai_plugin_runtime::{
    Capabilities, InvocationContext, PluginManifest, PluginPackage, PluginRuntime,
};
use serde_json::{json, Value};
use std::{collections::BTreeMap, sync::Arc, time::Duration};

/// 【公共网络测试】【外部实例】以非内置身份调用正式网络宿主
/// @param granted 是否授予任意来源只读权限
/// @returns 注册文本和二进制请求工具的独立 Lua 实例
fn external(granted: bool) -> PluginRuntime {
    let manifest = PluginManifest::parse(&json!({
        "api_version":1,"id":"external-http","version":"1.0.0","name":"External HTTP",
        "description":"Public HTTP contract","entry":"init.lua","capabilities":{"http_read_any":true}
    }).to_string()).unwrap();
    let source = r#"
        sai.register_tool({name='text',description='Text request',parameters={type='object'},
            execute=function(args) return sai.http.request(args) end})
        sai.register_tool({name='binary',description='Binary request',parameters={type='object'},
            execute=function(args)
                local response = sai.binary.request(args)
                local document = response.body:document({max_chars=32})
                response.body:close()
                return {status=response.status,text=document.text}
            end})
    "#;
    let package = PluginPackage::new(
        manifest,
        BTreeMap::from([("init.lua".into(), source.into())]),
    )
    .unwrap();
    PluginRuntime::load(
        package,
        json!({}),
        Capabilities {
            http_read_any: granted,
            ..Default::default()
        },
        Arc::new(SaiPluginHost),
    )
    .unwrap()
}

/// 【公共网络测试】【独立授权】普通外部包不能因声明或只读调用自动获得本地服务访问
/// @returns 无，授权后文本与二进制接口均可访问用户指定来源
#[tokio::test]
async fn external_arbitrary_reads_require_grants_before_connecting() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    for tool in ["text", "binary"] {
        let denied = external(false)
            .call_tool(tool, json!({"url":url}), InvocationContext::default())
            .await;
        assert!(format!("{:#}", denied.unwrap_err()).contains("origin is not allowed"));
    }
    assert!(
        tokio::time::timeout(Duration::from_millis(20), listener.accept())
            .await
            .is_err()
    );
    for tool in ["text", "binary"] {
        let (origin, task) = server(vec![response(200, "", b"public contract")]).await;
        let result = external(true)
            .call_tool(tool, json!({"url":origin}), InvocationContext::default())
            .await
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&result).unwrap()["text"],
            "public contract"
        );
        assert_eq!(task.await.unwrap().len(), 1);
    }
}

/// 【公共网络测试】【重定向凭据】任意来源授权不允许向新来源转发初始凭据头
/// @returns 无，文本与二进制请求只保留明确允许跨来源的普通头
#[tokio::test]
async fn arbitrary_origin_redirects_strip_sensitive_headers() {
    for tool in ["text", "binary"] {
        let (target, target_task) = server(vec![response(200, "", b"arrived")]).await;
        let (origin, origin_task) = server(vec![response(
            302,
            &format!("Location: {target}/next\r\n"),
            b"",
        )])
        .await;
        external(true).call_tool(tool,json!({"url":origin,"headers":{
            "Authorization":"Bearer fixture-token","X-Custom-Secret":"fixture-secret","Accept":"text/plain"
        }}),InvocationContext::default()).await.unwrap();
        let initial = origin_task.await.unwrap().join("").to_lowercase();
        assert!(initial.contains("fixture-token") && initial.contains("fixture-secret"));
        let final_request = target_task.await.unwrap().join("").to_lowercase();
        assert!(
            !final_request.contains("fixture-token") && !final_request.contains("fixture-secret")
        );
        assert!(final_request.contains("accept: text/plain"));
    }
}

/// 【公共网络测试】【文本响应选项】零次跳转阻止后续连接，错误正文选项不放宽成功正文上限
/// @returns 无，错误状态保留且没有读取超大正文
#[tokio::test]
async fn text_http_obeys_redirect_and_error_body_limits() {
    let (origin, task) = server(vec![response(302, "Location: /next\r\n", b"")]).await;
    let request: HttpRequest =
        serde_json::from_value(json!({"url":origin,"max_redirects":0})).unwrap();
    let error = SaiPluginHost
        .http(
            request,
            Capabilities {
                http_read_any: true,
                ..Default::default()
            },
            false,
        )
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("redirect limit"));
    assert_eq!(task.await.unwrap().len(), 1);
    for (status, read_error_body, succeeds) in [
        (404, false, true),
        (503, false, true),
        (404, true, false),
        (200, false, false),
    ] {
        let wire = format!(
            "HTTP/1.1 {status} Fixture\r\nContent-Length: 10000\r\nConnection: close\r\n\r\n"
        )
        .into_bytes();
        let (origin, task) = server(vec![wire]).await;
        let result = external(true)
            .call_tool(
                "text",
                json!({"url":origin,"max_bytes":16,"read_error_body":read_error_body}),
                InvocationContext::default(),
            )
            .await;
        assert_eq!(result.is_ok(), succeeds, "{result:?}");
        if let Ok(output) = result {
            let output: Value = serde_json::from_str(&output).unwrap();
            assert_eq!(output["status"], status);
            assert_eq!(output["text"], "");
        }
        task.await.unwrap();
    }
}
