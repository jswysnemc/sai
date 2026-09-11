mod common;

use common::runtime;
use sai_plugin_runtime::{EventContext, EventKind, InvocationContext};
use serde_json::{json, Value};

/// 【JSON 输入测试】【整数类型】无损查询区分整数、浮点表示及非数值，覆盖完整 64 位范围
/// @returns 无；大整数返回十进制文本，浮点数和其他类型返回 nil
#[tokio::test]
async fn json_integer_preserves_native_number_kinds() {
    let plugin = runtime(
        r#"
        --- 【JSON 输入测试】【读取】查询原参数中的 n，避免 Lua 数字转换影响类型
        --- @param args table 原工具参数
        --- @param ctx table 当前回调上下文
        --- @return table 原始整数文本或 JSON null
        local function read(args, ctx)
            return {integer=ctx.json_integer("/n") or sai.json.null}
        end
        sai.register_tool({name="read",description="Read integer",parameters={type="object"},execute=read})
    "#,
    );
    for (raw, expected) in [
        ("{}", None),
        (r#"{"n":0}"#, Some("0")),
        (r#"{"n":3}"#, Some("3")),
        (r#"{"n":-3}"#, Some("-3")),
        (r#"{"n":9007199254740993}"#, Some("9007199254740993")),
        (r#"{"n":9223372036854775807}"#, Some("9223372036854775807")),
        (r#"{"n":9223372036854775808}"#, Some("9223372036854775808")),
        (
            r#"{"n":18446744073709551615}"#,
            Some("18446744073709551615"),
        ),
        (
            r#"{"n":-9223372036854775808}"#,
            Some("-9223372036854775808"),
        ),
        (r#"{"n":3.0}"#, None),
        (r#"{"n":3e0}"#, None),
        (r#"{"n":-0}"#, None),
        (r#"{"n":1.8446744073709552e19}"#, None),
        (r#"{"n":18446744073709551616}"#, None),
        (r#"{"n":-9223372036854775809}"#, None),
        (r#"{"n":"3"}"#, None),
        (r#"{"n":true}"#, None),
        (r#"{"n":null}"#, None),
        (r#"{"n":[]}"#, None),
        (r#"{"n":{}}"#, None),
    ] {
        let output = plugin
            .call_tool(
                "read",
                serde_json::from_str(raw).unwrap(),
                InvocationContext::default(),
            )
            .await
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&output).unwrap(),
            json!({"integer":expected}),
            "{raw}"
        );
    }
}

/// 【JSON 输入测试】【路径快照】JSON Pointer 支持对象、数组与转义，Lua 修改参数不改变原输入
/// @returns 无；仅存在且以整数表示的节点返回文本
#[tokio::test]
async fn json_integer_reads_nested_pointers_from_the_original_snapshot() {
    let plugin = runtime(
        r#"
        --- 【JSON 输入测试】【快照读取】修改转换后的参数，再查询原始 JSON 节点
        --- @param args table 包含嵌套数字的参数
        --- @param ctx table 当前回调上下文
        --- @return table 各路径查询结果
        local function read(args, ctx)
            args.items[1] = 99
            local result = {}
            for _, pointer in ipairs({"/items/0", "/items/1", "/a~1b/~0/", "/", "", "/items/2", "/missing"}) do
                result[pointer] = ctx.json_integer(pointer) or sai.json.null
            end
            return result
        end
        sai.register_tool({name="read",description="Read pointers",parameters={type="object"},execute=read})
    "#,
    );
    let output = plugin
        .call_tool(
            "read",
            json!({
                "items":[1,2.0], "a/b":{"~":{"":u64::MAX}}, "":-7,
            }),
            InvocationContext::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&output).unwrap(),
        json!({
            "/items/0":"1", "/items/1":null, "/a~1b/~0/":"18446744073709551615", "/":"-7",
            "":null, "/items/2":null, "/missing":null,
        })
    );
}

/// 【JSON 输入测试】【查询有效期】保存的查询函数在回调结束后失效，后续工具使用独立输入
/// @returns 无；旧上下文不能读取新调用，命令不提供 JSON 参数查询
#[tokio::test]
async fn json_integer_expires_without_retaining_the_input() {
    let plugin = runtime(
        r#"
        local previous
        --- 【JSON 输入测试】【隔离读取】检查旧函数失效，再保存本次原输入查询
        --- @param args table 包含整数 n
        --- @param ctx table 当前回调上下文
        --- @return string 本次整数文本
        local function read(args, ctx)
            if previous then
                local ok, message = pcall(previous, "/n")
                assert(not ok and tostring(message):find("expired"))
            end
            previous = ctx.json_integer
            if args.fail then error("fixture callback failed") end
            return ctx.json_integer("/n")
        end
        --- 【JSON 输入测试】【命令隔离】纯文本命令不暴露前次工具的 JSON 查询
        --- @param args string 原始命令文本
        --- @param ctx table 当前命令上下文
        --- @return boolean 查询失效且没有 JSON 输入时为 true
        local function expired(args, ctx)
            assert(ctx.json_integer == nil)
            local ok, message = pcall(previous, "/n")
            return not ok and tostring(message):find("expired") ~= nil
        end
        sai.register_tool({name="read",description="Read integer",parameters={type="object"},execute=read})
        sai.register_command({name="expired",description="Check lifetime",execute=expired})
    "#,
    );
    assert!(plugin
        .call_tool(
            "read",
            json!({"n":6,"fail":true}),
            InvocationContext::default()
        )
        .await
        .is_err());
    assert_eq!(
        plugin
            .call_command("expired", "", InvocationContext::default())
            .await
            .unwrap(),
        "true"
    );
    for number in [7, 8] {
        assert_eq!(
            plugin
                .call_tool("read", json!({"n":number}), InvocationContext::default())
                .await
                .unwrap(),
            number.to_string()
        );
        assert_eq!(
            plugin
                .call_command("expired", "", InvocationContext::default())
                .await
                .unwrap(),
            "true"
        );
    }
}

/// 【JSON 输入测试】【事件输入】事件查询针对数据根节点，连续事件使用各自快照
/// @returns 无；事件整数精确保留，浮点值与空值不视为整数
#[tokio::test]
async fn json_integer_supports_event_data() {
    let plugin = runtime(
        r#"
        --- 【JSON 输入测试】【事件查询】读取事件根节点的原始整数
        --- @param data any 当前事件数据
        --- @param ctx table 当前事件上下文
        --- @return string|nil 整数文本，其他输入为空
        local function read(data, ctx)
            return ctx.json_integer("")
        end
        sai.on("turn_end", read)
    "#,
    );
    for (data, expected) in [
        (json!(u64::MAX), json!(u64::MAX.to_string())),
        (json!(3.0), Value::Null),
        (Value::Null, Value::Null),
    ] {
        let output = plugin
            .emit(
                EventKind::TurnEnd,
                EventContext {
                    data,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(output, vec![expected]);
    }
}

/// 【JSON 输入测试】【路径校验】非法类型、错误转义与过长路径不能绕过查询边界
/// @returns 无；被拒绝后同一回调仍能读取合法整数
#[tokio::test]
async fn json_integer_validates_pointer_types_and_recovers() {
    let plugin = runtime(
        r#"
        --- 【JSON 输入测试】【错误路径】在同次调用中验证错误参数与恢复
        --- @param args table 包含整数 n
        --- @param ctx table 当前回调上下文
        --- @return string 合法查询结果
        local function read(args, ctx)
            for _, pointer in ipairs({3, true, {}, "n", "/~", "/~2", "/" .. string.rep("x", 4096)}) do
                assert(not pcall(ctx.json_integer, pointer))
            end
            assert(not pcall(ctx.json_integer))
            return ctx.json_integer("/n")
        end
        sai.register_tool({name="read",description="Check pointers",parameters={type="object"},execute=read})
    "#,
    );
    assert_eq!(
        plugin
            .call_tool("read", json!({"n":7}), InvocationContext::default())
            .await
            .unwrap(),
        "7"
    );
}

/// 【JSON 输入测试】【共用预算】路径扫描计入原生计算预算，超限后下一次请求恢复
/// @returns 无；合法长路径不能绕过指令限制，短路径可在新回调中正常查询
#[tokio::test]
async fn json_integer_charges_pointer_work_to_the_shared_budget() {
    let mut source = common::package(
        r#"
        --- 【JSON 输入测试】【路径预算】按请求选择长路径或正常查询
        --- @param args table heavy 表示执行长路径扫描
        --- @param ctx table 当前回调上下文
        --- @return string|nil 当前路径的原始整数
        local function read(args, ctx)
            if args.heavy then return ctx.json_integer("/" .. string.rep("x", 4095)) end
            return ctx.json_integer("/n")
        end
        sai.register_tool({name="read",description="Read with budget",parameters={type="object"},execute=read})
    "#,
    );
    source.manifest.limits.instructions = 1000;
    let plugin = sai_plugin_runtime::PluginRuntime::load(
        source,
        json!({}),
        Default::default(),
        std::sync::Arc::new(common::RecordingHost::default()),
    )
    .unwrap();
    let error = plugin
        .call_tool("read", json!({"heavy":true}), InvocationContext::default())
        .await
        .unwrap_err();
    assert!(
        format!("{error:#}").contains("instruction budget"),
        "{error:#}"
    );
    assert_eq!(
        plugin
            .call_tool("read", json!({"n":9}), InvocationContext::default())
            .await
            .unwrap(),
        "9"
    );
}
