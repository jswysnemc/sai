#[path = "binary/support.rs"]
mod support;

use serde_json::json;
use std::sync::Arc;
use support::*;

/// 【二进制测试】【字节边界】原始读取保留零字节和无效 UTF-8，限制每次复制长度并校验偏移。
/// @returns 无；所有断言经过实际 Lua 缓冲入口
#[tokio::test]
async fn raw_bytes_are_exact_bounded_and_zero_based() {
    let host = Arc::new(Host::default());
    *host.body.lock().unwrap() = vec![0, 255, 128, b'a', b'b'];
    let plugin = runtime(
        r#"
        sai.register_tool({name='run',description='Inspect bytes',parameters={type='object'},execute=function()
            local b=sai.binary.request({url='https://example.test'}).body
            assert(b:bytes(0,3)==string.char(0,255,128))
            assert(b:bytes(3,1024)=='ab')
            assert(b:bytes(5,10)=='' and b:bytes(100,10)=='' and b:bytes(0,0)=='')
            for _,range in ipairs({{-1,1},{0,-1},{0,1025},{0,1.5}}) do
                assert(not pcall(b.bytes,b,range[1],range[2]))
            end
            b:close()
            local ok,err=pcall(b.bytes,b,0,1)
            assert(not ok and tostring(err):find('closed',1,true))
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

/// 【二进制测试】【标准摘要】采用公开 SHA-256 向量，关闭或跨回调句柄不能继续读取摘要或原始字节。
/// @returns 无；相同实例在旧句柄失效后仍能处理新缓冲
#[tokio::test]
async fn hashes_match_standard_vectors_and_handles_expire() {
    let plugin = runtime(
        r#"
        local old
        sai.register_tool({name='run',description='Hash bytes',parameters={type='object'},execute=function()
            if old then
                local bytes_ok,bytes_error=pcall(old.bytes,old,0,1)
                local hash_ok,hash_error=pcall(old.sha256,old)
                assert(not bytes_ok and tostring(bytes_error):find('expired',1,true))
                assert(not hash_ok and tostring(hash_error):find('expired',1,true))
            end
            local empty=sai.binary.decode_base64('')
            assert(empty:sha256()=='e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855')
            empty:close()
            assert(not pcall(empty.sha256,empty))
            old=sai.binary.decode_base64('YWJj')
            return old:sha256()
        end})
        "#,
        Arc::new(Host::default()),
        capabilities(),
        |_| {},
    );
    for _ in 0..2 {
        assert_eq!(
            plugin
                .call_tool("run", json!({}), context(false))
                .await
                .unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}

/// 【二进制测试】【计算预算】摘要消耗系统调用额度，修改 Lua 限制副本不能扩大预算。
/// @returns 无；新回调重新获得同样的有限预算
#[tokio::test]
async fn repeated_hashing_obeys_the_callback_budget() {
    let plugin = runtime(
        r#"
        sai.register_tool({name='run',description='Hash budget',parameters={type='object'},execute=function()
            sai.limits.system_calls=1000
            local b=sai.binary.decode_base64('YWJj')
            b:sha256()
            local ok,err=pcall(b.sha256,b)
            assert(not ok)
            return tostring(err)
        end})
        "#,
        Arc::new(Host::default()),
        capabilities(),
        |limits| limits.system_calls = 2,
    );
    for _ in 0..2 {
        assert!(plugin
            .call_tool("run", json!({}), context(false))
            .await
            .unwrap()
            .contains("budget exceeded"));
    }
}
