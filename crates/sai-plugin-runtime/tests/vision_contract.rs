#[path = "binary/support.rs"]
mod binary;
mod common;
#[path = "common/vision.rs"]
mod vision;

use sai_plugin_runtime::{Capabilities, PluginRuntime};
use serde_json::{json, Value};
use std::sync::{atomic::Ordering, Arc};
use vision::*;

/// 【视觉测试】【独立授权】文本、网络或终端权限均不能代替视觉能力，声明和授权缺一不可。
/// @returns 无；未授权调用不得抵达任何视觉宿主入口
#[tokio::test]
async fn vision_needs_its_own_declared_and_granted_capability() {
    for (declared, granted) in [(false, true), (true, false), (true, true)] {
        let mut package = common::package(
            r#"
            sai.register_tool({name='run',description='Vision authorization',parameters={type='object'},execute=function()
                local info_ok=pcall(sai.vision.info)
                local b=sai.binary.decode_base64('YWJj')
                local image_ok=pcall(b.analyze_image,b,{prompt='describe',mime_type='image/png'})
                return {info=info_ok,image=image_ok}
            end})
        "#,
        );
        let mut caps: Capabilities =
            serde_json::from_value(json!({"model":true,"http":["https://example.test"],
            "binary":{"public_downloads":true,"display_images":true}}))
            .unwrap();
        caps.vision = declared;
        package.manifest.capabilities = caps.clone();
        caps.vision = granted;
        let plugin = PluginRuntime::load(
            package,
            json!({}),
            caps,
            Arc::new(common::RecordingHost::default()),
        )
        .unwrap();
        let services = Arc::new(Services::default());
        let result: Value = serde_json::from_str(
            &plugin
                .call_tool("run", json!({}), context(services.clone()))
                .await
                .unwrap(),
        )
        .unwrap();
        let expected = declared && granted;
        assert_eq!(result, json!({"info":expected,"image":expected}));
        assert_eq!(
            services.info_calls.load(Ordering::SeqCst),
            usize::from(expected)
        );
        assert_eq!(
            services.requests.lock().unwrap().len(),
            usize::from(expected)
        );
    }
    let declared = capabilities();
    assert!(!Capabilities {
        vision: true,
        ..Default::default()
    }
    .is_subset(&Capabilities::default()));
    assert!(declared.is_subset(&declared));
    assert!(serde_json::from_value::<Capabilities>(json!({"vision":"true"})).is_err());
}

/// 【视觉测试】【非法参数】拒绝伪造模型、地址、路径、凭据和超长提示词，空图不能发送。
/// @returns 无；所有非法请求在宿主请求前失败
#[tokio::test]
async fn invalid_vision_options_are_rejected_before_the_host_is_called() {
    let plugin = runtime(
        r#"
        sai.register_tool({name='run',description='Vision validation',parameters={type='object'},execute=function()
            local b=sai.binary.decode_base64('YWJj')
            for _,name in ipairs({'provider_id','model','url','base_url','path','image','api_key','tools','headers'}) do
                local request={prompt='describe',mime_type='image/png'}
                request[name]='forged'
                assert(not pcall(b.analyze_image,b,request),name)
            end
            for _,request in ipairs({
                {prompt='',mime_type='image/png'}, {prompt='   ',mime_type='image/png'},
                {prompt='ok',mime_type='text/plain'}, {prompt='ok',mime_type='image/svg+xml'},
                {prompt='ok',mime_type='image/png',timeout_ms=-1},
                {prompt=string.rep('x',2048),mime_type='image/png'},
            }) do assert(not pcall(b.analyze_image,b,request)) end
            local empty=sai.binary.decode_base64('')
            assert(not pcall(empty.analyze_image,empty,{prompt='ok',mime_type='image/png'}))
            b:close()
            assert(not pcall(b.analyze_image,b,{prompt='ok',mime_type='image/png'}))
            return 'ok'
        end})
    "#,
        capabilities(),
        |limits| limits.output_bytes = 1024,
    );
    let services = Arc::new(Services::default());
    assert_eq!(
        plugin
            .call_tool("run", json!({}), context(services.clone()))
            .await
            .unwrap(),
        "ok"
    );
    assert!(services.requests.lock().unwrap().is_empty());
}

/// 【视觉测试】【图片上限】十 MiB 图片可发送，超过一字节时必须在模型请求前拒绝。
/// @returns 无；图片通过二进制缓冲进入实际绑定，不经过 Lua 文本输出
#[tokio::test]
async fn image_size_limit_is_enforced_on_actual_buffer_bytes() {
    for bytes in [10 * 1024 * 1024, 10 * 1024 * 1024 + 1] {
        let host = Arc::new(binary::Host::default());
        *host.body.lock().unwrap() = vec![0; bytes];
        let mut package = common::package(
            r#"
            sai.register_tool({name='run',description='Image limit',parameters={type='object'},execute=function()
                local b=sai.binary.request({url='https://example.test',max_bytes=12000000}).body
                local ok,response=pcall(b.analyze_image,b,{prompt='describe',mime_type='image/png'})
                return {ok=ok,value=ok and response.model or tostring(response)}
            end})
        "#,
        );
        let caps: Capabilities =
            serde_json::from_value(json!({"vision":true,"http":["https://example.test"]})).unwrap();
        package.manifest.capabilities = caps.clone();
        let plugin = PluginRuntime::load(package, json!({}), caps, host).unwrap();
        let services = Arc::new(Services::default());
        let result: Value = serde_json::from_str(
            &plugin
                .call_tool("run", json!({}), context(services.clone()))
                .await
                .unwrap(),
        )
        .unwrap();
        let allowed = bytes == 10 * 1024 * 1024;
        assert_eq!(result["ok"], allowed);
        assert_eq!(
            services.requests.lock().unwrap().len(),
            usize::from(allowed)
        );
        if !allowed {
            assert!(result["value"].as_str().unwrap().contains("10485760"));
        }
    }
}

/// 【视觉测试】【共享预算】视觉与文本共用请求次数，Lua 修改额度无效，新回调恢复原预算。
/// @returns 无；每轮仅有一个实际模型请求
#[tokio::test]
async fn vision_and_text_share_a_non_forgeable_request_budget() {
    let plugin = runtime(
        r#"
        sai.register_tool({name='run',description='Shared budget',parameters={type='object'},execute=function(args)
            sai.limits.model_requests=1000
            local b=sai.binary.decode_base64('YWJj')
            local request={prompt='describe',mime_type='image/png'}
            local text={messages={{role='user',content='question'}}}
            if args.vision_first then b:analyze_image(request) else sai.model.complete(text) end
            local ok,err
            if args.vision_first then ok,err=pcall(sai.model.complete,text)
            else ok,err=pcall(b.analyze_image,b,request) end
            assert(not ok)
            return tostring(err)
        end})
    "#,
        capabilities(),
        |limits| limits.model_requests = 1,
    );
    let services = Arc::new(Services::default());
    for first in [true, false] {
        assert!(plugin
            .call_tool(
                "run",
                json!({"vision_first":first}),
                context(services.clone())
            )
            .await
            .unwrap()
            .contains("budget exceeded"));
    }
    assert_eq!(services.requests.lock().unwrap().len(), 1);
    assert_eq!(services.text.models.lock().unwrap().len(), 1);
}

/// 【视觉测试】【输出限制】宿主返回的超长正文不能突破 Lua 输出预算，关闭状态保留空模型信息。
/// @returns 无；错误能由插件捕获，不中断实例后续调用
#[tokio::test]
async fn oversized_vision_responses_are_catchable_and_disabled_info_is_null() {
    let plugin = runtime(
        r#"
        sai.register_tool({name='run',description='Vision response limit',parameters={type='object'},execute=function()
            assert(sai.vision.info()==sai.json.null)
            local b=sai.binary.decode_base64('YWJj')
            local ok,err=pcall(b.analyze_image,b,{prompt='describe',mime_type='image/png'})
            assert(not ok)
            return tostring(err)
        end})
    "#,
        capabilities(),
        |limits| limits.output_bytes = 1024,
    );
    let services = Arc::new(Services::default());
    services.disabled.store(true, Ordering::SeqCst);
    *services.response.lock().unwrap() = "x".repeat(2048);
    assert!(plugin
        .call_tool("run", json!({}), context(services))
        .await
        .unwrap()
        .contains("vision response exceeds size limit"));
}
