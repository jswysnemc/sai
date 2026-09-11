#[path = "binary/conditional_support.rs"]
mod support;

use serde_json::json;
use std::sync::Arc;
use support::*;

/// 【二进制构造测试】【初始化拦截】原始字节和已有解码入口都只能在回调中使用
/// @returns 无；初始化失败不进入宿主
#[test]
fn buffer_constructors_reject_initialization() {
    for source in ["sai.binary.from_bytes('')", "sai.binary.decode_base64('')"] {
        let error = load(
            source,
            Arc::new(Host::default()),
            capabilities(),
            capabilities(),
            |_| {},
        )
        .err()
        .unwrap();
        assert!(
            format!("{error:#}").contains("only available during plugin callbacks"),
            "{error:#}"
        );
    }
}

/// 【二进制构造测试】【输入限制】只接受原始字符串且不超过文字输入上限
/// @returns 无；空字符串与上限边界保持可用，错误输入不能消耗字节预算
#[tokio::test]
async fn raw_constructor_rejects_coercion_and_oversized_input() {
    let plugin = runtime(
        r#"
        --- 【二进制构造测试】【类型校验】拒绝隐式字符串转换和超限分配
        --- @return integer 合法边界字节数
        local function run()
            for _, value in ipairs({42,true,{},function() end}) do
                assert(not pcall(sai.binary.from_bytes,value))
            end
            assert(not pcall(sai.binary.from_bytes,nil))
            assert(not pcall(sai.binary.from_bytes,string.rep("x",1025)))
            assert(sai.binary.from_bytes(""):len()==0)
            return sai.binary.from_bytes(string.rep("x",1024)):len()
        end
        sai.register_tool({name="run",description="Raw inputs",parameters={type="object"},execute=run})
    "#,
        Arc::new(Host::default()),
        |limits| {
            limits.output_bytes = 1024;
            limits.binary_bytes = 4096;
        },
    );
    assert_eq!(
        plugin
            .call_tool("run", json!({}), context(false))
            .await
            .unwrap(),
        "1024"
    );
}

/// 【二进制构造测试】【共用预算】原始构造和解码句柄共用总量，关闭后立即归还空间
/// @returns 无；保留的旧句柄不能跨回调继续写入
#[tokio::test]
async fn raw_buffers_share_budget_and_expire_after_success_or_failure() {
    let host = Arc::new(Host::default());
    let plugin = runtime(
        r#"
        local saved
        --- 【二进制构造测试】【句柄代次】验证关闭、错误和正常退出后的预算
        --- @param args table 是否在本次回调失败
        --- @return integer 可用边界大小
        local function run(args)
            if saved then
                local ok,message=pcall(saved.write_if,saved,"output/a",nil)
                assert(not ok and tostring(message):find("expired",1,true),tostring(message))
                saved:close()
            end
            local first=sai.binary.from_bytes(string.rep("x",512))
            local second=sai.binary.decode_base64(string.rep("YWFh",170).."YWE=")
            assert(second:len()==512)
            assert(not pcall(sai.binary.from_bytes,"x"))
            assert(sai.binary.from_bytes(""):len()==0)
            first:close();first:close();second:close()
            local ok,message=pcall(first.write_if,first,"output/a",nil)
            assert(not ok and tostring(message):find("closed",1,true),tostring(message))
            saved=sai.binary.from_bytes(string.rep("x",1024))
            if args.fail then error("fixture failure") end
            return saved:len()
        end
        sai.register_tool({name="run",description="Raw lifetime",access="writes",parameters={type="object"},execute=run})
    "#,
        host.clone(),
        |limits| limits.binary_bytes = 1024,
    );
    for fail in [false, true, false] {
        let result = plugin
            .call_tool("run", json!({"fail":fail}), context(true))
            .await;
        if fail {
            assert!(format!("{:#}", result.unwrap_err()).contains("fixture failure"));
        } else {
            assert_eq!(result.unwrap(), "1024");
        }
    }
    assert!(host.writes.lock().unwrap().is_empty());
}

/// 【二进制构造测试】【调用额度】原始构造和已有解码共用系统调用次数
/// @returns 无；空缓冲也计费，下一回调重新获得额度
#[tokio::test]
async fn constructors_charge_the_shared_system_call_limit() {
    let plugin = runtime(
        r#"
        --- 【二进制构造测试】【额度检查】三个构造之后拒绝第四个构造
        --- @return string 校验结果
        local function run()
            sai.binary.from_bytes("a")
            sai.binary.decode_base64("Yg==")
            sai.binary.from_bytes("")
            local ok,message=pcall(sai.binary.from_bytes,"")
            assert(not ok and tostring(message):find("call budget exceeded",1,true),tostring(message))
            return "limited"
        end
        sai.register_tool({name="run",description="Raw calls",parameters={type="object"},execute=run})
    "#,
        Arc::new(Host::default()),
        |limits| limits.system_calls = 3,
    );
    for _ in 0..2 {
        assert_eq!(
            plugin
                .call_tool("run", json!({}), context(false))
                .await
                .unwrap(),
            "limited"
        );
    }
}
