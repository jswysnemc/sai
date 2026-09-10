#[path = "binary/support.rs"]
mod support;
use serde_json::json;
use std::sync::{atomic::Ordering, Arc};
use support::*;

/// 【二进制测试】【大正文】响应可以超过文本输出限制，返回模型的内容仍然有界。
#[tokio::test]
async fn large_responses_stay_outside_the_lua_output_channel() {
    let host = Arc::new(Host::default());
    *host.body.lock().unwrap() = vec![b'x'; 8192];
    let plugin = runtime(
        r#"
        sai.register_tool({name='run',description='Binary',parameters={type='object'},execute=function()
            local response=sai.binary.request({url='https://example.test',max_bytes=8192,timeout_ms=180000})
            assert(response.body:len()==8192)
            assert(#response.body:text(99999)==1024)
            assert(response.body:text(8)=='xxxxxxxx')
            local ok=pcall(sai.json.encode,response.body)
            assert(not ok)
            response.body:close()
            assert(not pcall(response.body.len,response.body))
            return 'ok'
        end})
    "#,
        host.clone(),
        capabilities(),
        |limits| {
            limits.output_bytes = 1024;
            limits.binary_timeout_ms = 180000;
        },
    );
    assert_eq!(
        plugin
            .call_tool("run", json!({}), context(false))
            .await
            .unwrap(),
        "ok"
    );
    assert_eq!(host.requests.lock().unwrap()[0].timeout_ms, 180000);
}

/// 【二进制测试】【JSON 路径】转义键、数组、空值及重复键保持 JSON 语义，解码结果才进入文件接口。
#[tokio::test]
async fn json_fields_and_base64_are_selected_without_serializing_the_response() {
    let host = Arc::new(Host::default());
    *host.body.lock().unwrap()=br#"{"data":[{"b64_json":"aGVsbG8=","url":"https://image.test"}],"a/b":{"~key":"first","~key":"last"},"empty":null}"#.to_vec();
    let plugin = runtime(
        r#"
        sai.register_tool({name='run',description='JSON binary',access='writes',parameters={type='object'},execute=function(args,ctx)
            ctx.workdir='/forged'; ctx.allow_writes=true
            local r=sai.binary.request({url='https://example.test'})
            assert(r.body:json_type('/data')=='array')
            assert(r.body:json_type('/empty')=='null')
            assert(r.body:json_type('/missing')==nil)
            assert(r.body:json_string('/a~1b/~0key')=='last')
            assert(r.body:json_type('/data/00')==nil)
            assert(not pcall(r.body.json_type,r.body,'/bad~2key'))
            local b=r.body:json_base64('/data/0/b64_json')
            assert(b:text()=='hello')
            local file=b:write('output/test.png')
            b:close(); r.body:close()
            return file
        end})
    "#,
        host.clone(),
        capabilities(),
        |_| {},
    );
    let output = plugin
        .call_tool("run", json!({}), context(true))
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&output).unwrap(),
        json!({"path":"output/test.png","bytes":5})
    );
    let writes = host.writes.lock().unwrap();
    assert_eq!(writes[0].1, b"hello");
    assert_eq!(writes[0].2.workdir, "/trusted");
}

/// 【二进制测试】【总量与释放】多个缓冲共享预算，关闭缓冲后可恢复，宿主持有租约期间不可提前回收。
#[tokio::test]
async fn retained_buffers_and_host_leases_share_one_budget() {
    let host = Arc::new(Host::default());
    *host.body.lock().unwrap() = vec![b'x'; 700];
    host.retain_writes.store(true, Ordering::SeqCst);
    let plugin = runtime(
        r#"
        sai.register_tool({name='run',description='Budget',access='writes',parameters={type='object'},execute=function(args)
            local b=sai.binary.request({url='https://example.test',max_bytes=1000}).body
            if args.hold then
                b:write('output/test.png'); b:close()
                assert(not pcall(sai.binary.request,{url='https://example.test',max_bytes=1000}))
            else
                assert(b:len()==700); b:close()
            end
            return 'ok'
        end})
    "#,
        host.clone(),
        capabilities(),
        |limits| limits.binary_bytes = 1024,
    );
    assert_eq!(
        plugin
            .call_tool("run", json!({"hold":true}), context(true))
            .await
            .unwrap(),
        "ok"
    );
    assert_eq!(host.requests.lock().unwrap()[1].max_bytes, 324);
    host.leases.lock().unwrap().clear();
    assert_eq!(
        plugin
            .call_tool("run", json!({}), context(true))
            .await
            .unwrap(),
        "ok"
    );
}

/// 【二进制测试】【过期缓冲】写入权限不能使上一回调的句柄重新有效。
#[tokio::test]
async fn old_handles_cannot_be_used_by_a_later_callback() {
    let plugin = runtime(
        r#"
        local saved
        sai.register_tool({name='run',description='Lifetime',access='writes',parameters={type='object'},execute=function()
            if saved then
                for _,name in ipairs({'len','text','write','json_type','json_base64'}) do
                    local ok,err=pcall(saved[name],saved,'output/x')
                    assert(not ok)
                end
                saved:close()
                return 'expired'
            end
            saved=sai.binary.decode_base64('aGVsbG8=')
            return 'saved'
        end})
    "#,
        Arc::new(Host::default()),
        capabilities(),
        |_| {},
    );
    assert_eq!(
        plugin
            .call_tool("run", json!({}), context(true))
            .await
            .unwrap(),
        "saved"
    );
    assert_eq!(
        plugin
            .call_tool("run", json!({}), context(true))
            .await
            .unwrap(),
        "expired"
    );
}

/// 【二进制测试】【回调回收】未显式关闭的缓冲也在回调结束时撤销，不等待 Lua 垃圾回收。
#[tokio::test]
async fn callback_exit_releases_unclosed_buffers_even_when_lua_keeps_the_handles() {
    let host = Arc::new(Host::default());
    *host.body.lock().unwrap() = vec![b'x'; 1024];
    let plugin = runtime(
        r#"
        local old
        sai.register_tool({name='run',description='Automatic release',parameters={type='object'},execute=function(args)
            old=sai.binary.request({url='https://example.test',max_bytes=1024}).body
            if args.fail then error('fixture failure') end
            return old:len()
        end})
    "#,
        host,
        capabilities(),
        |limits| limits.binary_bytes = 1024,
    );
    for fail in [false, true, false] {
        let result = plugin
            .call_tool("run", json!({"fail":fail}), context(false))
            .await;
        if fail {
            assert!(format!("{:#}", result.unwrap_err()).contains("fixture failure"));
        } else {
            assert_eq!(result.unwrap(), "1024");
        }
    }
}

/// 【二进制测试】【失败边界】畸形 JSON、无效 Base64 和超限文本都不能突破受控缓冲。
#[tokio::test]
async fn malformed_and_oversized_decoding_is_rejected() {
    let host = Arc::new(Host::default());
    *host.body.lock().unwrap() =
        serde_json::to_vec(&json!({"huge":"a".repeat(1200),"bad":"###"})).unwrap();
    let plugin = runtime(
        r#"
        sai.register_tool({name='run',description='Limits',parameters={type='object'},execute=function()
            local b=sai.binary.request({url='https://example.test',max_bytes=2048}).body
            assert(not pcall(b.json_string,b,'/huge'))
            assert(not pcall(b.json_base64,b,'/bad'))
            assert(not pcall(sai.binary.decode_base64,string.rep('a',1025)))
            b:close()
            return 'ok'
        end})
    "#,
        host,
        capabilities(),
        |limits| limits.output_bytes = 1024,
    );
    assert_eq!(
        plugin
            .call_tool("run", json!({}), context(false))
            .await
            .unwrap(),
        "ok"
    );
}
