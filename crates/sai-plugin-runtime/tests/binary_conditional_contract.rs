mod common;
#[path = "binary/conditional_support.rs"]
mod support;

use common::runtime;
use sai_plugin_runtime::InvocationContext;
use serde_json::json;

/// 【条件写入测试】【原始字节构造】纯字节输入必须能创建受控缓冲，不经过文本编码转换
/// @returns 无；零字节和非 UTF-8 字节完整保留
#[tokio::test]
async fn raw_byte_constructor_preserves_original_bytes() {
    let plugin = runtime(
        r#"
        --- 【条件写入测试】【原始构造】检查原始 Lua 字节入口
        --- @return integer 完整缓冲长度
        local function run()
            assert(type(sai.binary.from_bytes)=="function", "raw byte constructor is missing")
            local raw = string.char(0,255,128,65,0)
            local data = sai.binary.from_bytes(raw)
            assert(data:bytes(0,5)==raw)
            return data:len()
        end
        sai.register_tool({name="run",description="Raw buffer",parameters={type="object"},execute=run})
    "#,
    );
    assert_eq!(
        plugin
            .call_tool("run", json!({}), InvocationContext::default())
            .await
            .unwrap(),
        "5"
    );
}

/// 【条件写入测试】【接口授权】缺少授权时必须由实际条件写入入口拒绝
/// @returns 无；接口存在且权限错误可由 Lua 捕获
#[tokio::test]
async fn conditional_file_writes_require_explicit_path_grants() {
    let plugin = runtime(
        r#"
        --- 【条件写入测试】【无授权调用】检查新句柄方法的实际权限边界
        --- @return string 拒绝结果
        local function run()
            local data = sai.binary.decode_base64("e30=")
            assert(type(data.write_if)=="function", "conditional write API is missing")
            local ok, message = pcall(data.write_if,data,"output/index.json",nil)
            assert(not ok and tostring(message):find("not allowed",1,true),tostring(message))
            return "denied"
        end
        sai.register_tool({name="run",description="Conditional write grant",parameters={type="object"},execute=run})
    "#,
    );
    assert_eq!(
        plugin
            .call_tool("run", json!({}), InvocationContext::default())
            .await
            .unwrap(),
        "denied"
    );
}

/// 【条件写入测试】【旧宿主兼容】未实现条件写入的宿主必须明确拒绝，不能退回无条件覆盖
/// @returns 无；相同 Lua 接口保留可捕获的不可用错误
#[tokio::test]
async fn hosts_without_conditional_support_do_not_fall_back_to_unconditional_writes() {
    let plugin = support::runtime(
        support::SOURCE,
        std::sync::Arc::new(common::RecordingHost::default()),
        |_| {},
    );
    let error = plugin
        .call_tool("run", json!({}), support::context(true))
        .await
        .unwrap_err();
    assert!(
        format!("{error:#}").contains("conditional binary file writing is unavailable"),
        "{error:#}"
    );
}

/// 【条件写入测试】【原始路径类型】无效 UTF-8、布尔值和缺失路径都不能隐式转成有效目标
/// @returns 无；全部输入在宿主调用前失败
#[tokio::test]
async fn conditional_paths_reject_missing_nonstring_and_invalid_utf8_values() {
    let host = std::sync::Arc::new(support::Host::default());
    let plugin = support::runtime(
        r#"
        --- 【条件写入测试】【严格路径】通过 Lua 原始值检查路径边界
        --- @return string 拒绝结果
        local function run()
            local data=sai.binary.from_bytes("x")
            assert(not pcall(data.write_if,data))
            for _,path in ipairs({false,true,42,{},string.char(255)}) do
                assert(not pcall(data.write_if,data,path,nil))
            end
            return "denied"
        end
        sai.register_tool({name="run",description="Strict write paths",access="writes",parameters={type="object"},execute=run})
    "#,
        host.clone(),
        |_| {},
    );
    assert_eq!(
        plugin
            .call_tool("run", json!({}), support::context(true))
            .await
            .unwrap(),
        "denied"
    );
    assert!(host.writes.lock().unwrap().is_empty());
}
