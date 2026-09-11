mod common;
#[path = "binary/read_support.rs"]
mod support;

use sai_plugin_runtime::InvocationContext;
use serde_json::json;
use std::sync::{atomic::Ordering, Arc};
use support::*;

/// 【二进制读取测试】【接口与授权】读取入口存在，但没有目录声明时必须在宿主前拒绝
/// @returns 无；读取失败可捕获，纯计算仍可继续
#[tokio::test]
async fn binary_file_reading_requires_an_explicit_read_grant() {
    let plugin = common::runtime(
        r#"
        --- 【二进制读取测试】【无授权探测】检查实际读取入口和拒绝原因
        --- @return string 授权检查成功时返回固定结果
        local function probe()
            assert(type(sai.binary.read_file) == "function", "binary file API is missing")
            local ok, message = pcall(sai.binary.read_file, "image.png")
            assert(not ok and tostring(message):find("not allowed"), tostring(message))
            return "denied"
        end
        sai.register_tool({name="probe",description="Read grant probe",parameters={type="object"},execute=probe})
    "#,
    );
    assert_eq!(
        plugin
            .call_tool("probe", json!({}), InvocationContext::default())
            .await
            .unwrap(),
        "denied"
    );
}

/// 【二进制读取测试】【授权交集】单独声明、单独授权和不同目录集合都不能开放读取
/// @returns 无；只有交集中的读取能力抵达宿主
#[tokio::test]
async fn binary_reads_use_the_declared_and_granted_read_path_intersection() {
    let other = serde_json::from_value(json!({"system":{"read_paths":["other"]}})).unwrap();
    for (declared, granted) in [
        (capabilities(), Default::default()),
        (Default::default(), capabilities()),
        (capabilities(), other),
    ] {
        let host = Arc::new(Host::default());
        let plugin = load(SOURCE, host.clone(), declared, granted, |_| {}).unwrap();
        let error = plugin
            .call_tool("read", json!({}), context())
            .await
            .unwrap_err();
        assert!(format!("{error:#}").contains("not allowed"), "{error:#}");
        assert!(host.reads.lock().unwrap().is_empty());
    }
    let host = Arc::new(Host::default());
    let plugin = runtime(
        r#"
        --- 【二进制读取测试】【上下文伪造】修改可见表不能改变宿主目录和权限
        --- @param args table 输入；ctx 为 Lua 上下文
        --- @return integer 文件长度
        local function read(args, ctx)
            ctx.workdir = "/forged"; ctx.allow_writes = true
            return sai.binary.read_file("allowed/image.bin"):len()
        end
        sai.register_tool({name="read",description="Trusted context",parameters={type="object"},execute=read})
    "#,
        host.clone(),
        |_| {},
    );
    assert_eq!(
        plugin
            .call_tool("read", json!({}), context())
            .await
            .unwrap(),
        "0"
    );
    let reads = host.reads.lock().unwrap();
    assert_eq!(reads[0].path, "allowed/image.bin");
    assert_eq!(reads[0].context.workdir, "/trusted");
    assert!(!reads[0].context.allow_writes);
    assert_eq!(
        reads[0].capabilities.system.read_paths,
        capabilities().system.read_paths
    );
}

/// 【二进制读取测试】【初始化隔离】加载脚本不能执行二进制文件读取
/// @returns 无；初始化失败且宿主没有收到请求
#[test]
fn binary_file_reads_are_unavailable_during_initialization() {
    let host = Arc::new(Host::default());
    assert!(load(
        "sai.binary.read_file('allowed/image.bin')",
        host.clone(),
        capabilities(),
        capabilities(),
        |_| {}
    )
    .is_err());
    assert!(host.reads.lock().unwrap().is_empty());
}

/// 【二进制读取测试】【严格参数】路径、选项类型和未知字段均在调用宿主前拒绝
/// @returns 无；控制字段、非法整数和非文本路径都不能发起 I/O
#[tokio::test]
async fn invalid_read_paths_and_options_never_reach_the_host() {
    let host = Arc::new(Host::default());
    let plugin = runtime(
        r#"
        --- 【二进制读取测试】【参数矩阵】检查不能静默转换的路径和读取选项
        --- @return string 验证结果
        local function probe()
            for _, path in ipairs({false, 7, {}, "", "../escape", "a/../b", "a\0b", "a\nb", "*.png", string.rep("a",4097), string.char(255)}) do
                assert(not pcall(sai.binary.read_file,path), "accepted invalid path")
            end
            assert(not pcall(sai.binary.read_file))
            for _, options in ipairs({false, 3, "", {workdir="/forged"}, {allow_writes=true}, {path="other"}, {lossy=true}, {1}, {[false]=1}}) do
                assert(not pcall(sai.binary.read_file,"allowed/a",options), "accepted invalid options")
            end
            for _, name in ipairs({"max_bytes", "timeout_ms"}) do
                for _, value in ipairs({0, -1, 1.5, math.huge, -math.huge, 0/0, "2", true, {}, sai.json.null}) do
                    assert(not pcall(sai.binary.read_file,"allowed/a",{[name]=value}), "accepted invalid limit")
                end
            end
            return "rejected"
        end
        sai.register_tool({name="probe",description="Invalid inputs",parameters={type="object"},execute=probe})
    "#,
        host.clone(),
        |_| {},
    );
    assert_eq!(
        plugin
            .call_tool("probe", json!({}), context())
            .await
            .unwrap(),
        "rejected"
    );
    assert!(host.reads.lock().unwrap().is_empty());
}

/// 【二进制读取测试】【默认与收窄】默认读取一 MiB，显式上限仍受剩余预算限制
/// @returns 无；有效整数选项没有扩大宿主额度
#[tokio::test]
async fn read_limits_default_to_one_mib_and_narrow_to_available_bytes() {
    let host = Arc::new(Host::default());
    *host.body.lock().unwrap() = vec![7; 16];
    let plugin = runtime(
        r#"
        --- 【二进制读取测试】【选项收窄】保留一个缓冲后检查后续读取的可用额度
        --- @return string 验证结果
        local function read()
            local first = sai.binary.read_file("allowed/a")
            local second = sai.binary.read_file("allowed/a",{max_bytes=math.maxinteger,timeout_ms=math.maxinteger})
            first:close(); second:close()
            local third = sai.binary.read_file("allowed/a",{max_bytes=16.0,timeout_ms=1000.0})
            assert(third:len()==16); third:close()
            return "ok"
        end
        sai.register_tool({name="read",description="Read limits",parameters={type="object"},execute=read})
    "#,
        host.clone(),
        |limits| limits.binary_bytes = 2 * 1024 * 1024,
    );
    assert_eq!(
        plugin
            .call_tool("read", json!({}), context())
            .await
            .unwrap(),
        "ok"
    );
    let reads = host.reads.lock().unwrap();
    assert_eq!(
        reads.iter().map(|read| read.max_bytes).collect::<Vec<_>>(),
        vec![1024 * 1024, 2 * 1024 * 1024 - 16, 16]
    );
}

/// 【二进制读取测试】【调用额度】读取和摘要共用系统次数，异常读取也消耗额度
/// @returns 无；超额请求不会抵达宿主，每次回调重新计数
#[tokio::test]
async fn file_reads_and_failed_reads_consume_the_shared_system_call_budget() {
    for fail in [false, true] {
        let host = Arc::new(Host::default());
        host.fail.store(fail, Ordering::SeqCst);
        let plugin = runtime(
            r#"
            --- 【二进制读取测试】【次数约束】即使捕获宿主错误，也不能重新获得调用额度
            --- @return string 验证结果
            local function read(args)
                local ok, data = pcall(sai.binary.read_file,"allowed/a")
                if args.fail then assert(not ok) else assert(ok); data:sha256() end
                local retry, message = pcall(sai.binary.read_file,"allowed/a")
                assert(not retry and tostring(message):find("system call budget exceeded",1,true), tostring(message))
                return "limited"
            end
            sai.register_tool({name="read",description="Call budget",parameters={type="object"},execute=read})
        "#,
            host.clone(),
            |limits| limits.system_calls = if fail { 1 } else { 2 },
        );
        for _ in 0..2 {
            assert_eq!(
                plugin
                    .call_tool("read", json!({"fail":fail}), context())
                    .await
                    .unwrap(),
                "limited"
            );
        }
        assert_eq!(host.reads.lock().unwrap().len(), 2);
    }
}

/// 【二进制读取测试】【默认宿主】未实现读取能力的宿主明确拒绝，不影响旧宿主加载
/// @returns 无；错误可重复捕获，预算没有泄漏
#[tokio::test]
async fn hosts_without_binary_file_support_fail_explicitly() {
    let plugin = runtime(
        SOURCE,
        Arc::new(common::RecordingHost::default()),
        |limits| limits.binary_bytes = 1024,
    );
    for _ in 0..2 {
        let error = plugin
            .call_tool("read", json!({}), context())
            .await
            .unwrap_err();
        assert!(
            format!("{error:#}").contains("binary file reading is unavailable"),
            "{error:#}"
        );
    }
}
