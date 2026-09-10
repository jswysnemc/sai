#[path = "binary/support.rs"]
mod support;
use sai_plugin_runtime::{Capabilities, PluginManifest, PluginPackage, PluginRuntime};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;
use support::*;

/// 【二进制测试】【分项授权】撤销下载、写入或展示不会改变其他能力，声明交集不能扩大权限。
#[test]
fn binary_grants_are_independent_and_default_to_denied() {
    let declared = capabilities();
    let granted: Capabilities =
        serde_json::from_value(json!({"binary":{"display_images":true,"write_paths":["outside"]}}))
            .unwrap();
    let effective = declared.intersection(&granted);
    assert!(effective.binary.display_images);
    assert!(!effective.binary.public_downloads);
    assert!(effective.binary.write_paths.is_empty());
    assert!(!granted.is_subset(&declared));
    assert!(effective.is_subset(&declared));
    assert!(Capabilities::default().binary.is_empty());
    assert!(serde_json::from_value::<Capabilities>(json!({"binary":{"unknown":true}})).is_err());
    let invalid: Capabilities =
        serde_json::from_value(json!({"binary":{"write_paths":["output/../private"]}})).unwrap();
    assert!(invalid.validate().is_err());
}

/// 【二进制测试】【只读回调】Lua 修改上下文不能获取写入权限，精确 API 写入也须经过宿主授权。
#[tokio::test]
async fn read_only_callbacks_cannot_write_files_or_post_requests() {
    let host = Arc::new(Host::default());
    let plugin = runtime(
        r#"
        sai.register_tool({name='run',description='Readonly',parameters={type='object'},execute=function(args,ctx)
            ctx.allow_writes=true
            local b=sai.binary.decode_base64('aGVsbG8=')
            local ok,err=pcall(b.write,b,'output/file')
            assert(not ok and tostring(err):find('read-only',1,true))
            ok,err=pcall(sai.binary.request,{url='https://example.test',method='POST'})
            assert(not ok and tostring(err):find('read-only',1,true))
            b:close()
            return 'ok'
        end})
    "#,
        host.clone(),
        capabilities(),
        |_| {},
    );
    assert_eq!(
        plugin
            .call_tool("run", json!({}), context(true))
            .await
            .unwrap(),
        "ok"
    );
    assert!(host.writes.lock().unwrap().is_empty());
    assert!(host.requests.lock().unwrap().is_empty());
}

/// 【二进制测试】【无凭据下载】公开下载授权不允许正文、认证头或其他方法。
#[tokio::test]
async fn anonymous_downloads_cannot_forward_credentials_or_bodies() {
    let host = Arc::new(Host::default());
    let plugin = runtime(
        r#"
        sai.register_tool({name='run',description='Download',access='writes',parameters={type='object'},execute=function()
            for _,request in ipairs({
                {url='https://image.test',method='POST'},
                {url='https://image.test',body='secret'},
                {url='https://image.test',headers={authorization='secret'}},
                {url='https://image.test',headers={cookie='secret'}},
            }) do assert(not pcall(sai.binary.download,request)) end
            local b=sai.binary.download({url='https://image.test'}).body
            b:close()
            return 'ok'
        end})
    "#,
        host.clone(),
        capabilities(),
        |_| {},
    );
    assert_eq!(
        plugin
            .call_tool("run", json!({}), context(true))
            .await
            .unwrap(),
        "ok"
    );
    assert_eq!(host.requests.lock().unwrap().len(), 1);
    let denied = runtime(
        r#"
        sai.register_tool({name='run',description='Denied',parameters={type='object'},execute=function()
            return sai.binary.download({url='https://image.test'})
        end})
    "#,
        host.clone(),
        Capabilities::default(),
        |_| {},
    );
    assert!(denied
        .call_tool("run", json!({}), context(true))
        .await
        .is_err());
    assert_eq!(host.requests.lock().unwrap().len(), 1);
}

/// 【二进制测试】【展示与写入授权】缺少独立授权时不进入宿主，写入模式也不能代替能力声明。
#[tokio::test]
async fn missing_binary_capabilities_stop_before_host_side_effects() {
    let host = Arc::new(Host::default());
    let plugin = runtime(
        r#"
        sai.register_tool({name='run',description='Denied',access='writes',parameters={type='object'},execute=function()
            local b=sai.binary.decode_base64('aGVsbG8=')
            assert(not pcall(b.write,b,'output/file'))
            assert(not pcall(sai.terminal.size))
            assert(not pcall(sai.terminal.display_image,'image.png','80x40'))
            b:close()
            return 'ok'
        end})
    "#,
        host.clone(),
        Capabilities::default(),
        |_| {},
    );
    assert_eq!(
        plugin
            .call_tool("run", json!({}), context(true))
            .await
            .unwrap(),
        "ok"
    );
    assert!(host.writes.lock().unwrap().is_empty());
    assert!(host.images.lock().unwrap().is_empty());
}

/// 【二进制测试】【加载阶段】包初始化阶段不能使用下载、展示或解码句柄入口。
#[test]
fn initialization_cannot_use_binary_host_services() {
    for expression in [
        "sai.binary.request({url='https://example.test'})",
        "sai.binary.download({url='https://example.test'})",
        "sai.binary.decode_base64('aGVsbG8=')",
        "sai.terminal.size()",
        "sai.terminal.display_image('image.png')",
    ] {
        let manifest=PluginManifest::parse(&json!({"api_version":1,"id":"init-test","version":"1.0.0",
            "name":"Init","description":"Init boundary","entry":"init.lua","capabilities":capabilities()}).to_string()).unwrap();
        let package = PluginPackage::new(
            manifest,
            BTreeMap::from([("init.lua".into(), expression.into())]),
        )
        .unwrap();
        assert!(
            PluginRuntime::load(
                package,
                json!({}),
                capabilities(),
                Arc::new(Host::default())
            )
            .is_err(),
            "{expression}"
        );
    }
}
