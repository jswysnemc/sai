mod common;

use common::{package, runtime, RecordingHost};
use sai_plugin_runtime::{Capabilities, InvocationContext, PluginRuntime};
use serde_json::{json, Value};
use std::sync::Arc;

const DECODE_TOOL: &str = r#"
sai.register_tool({name="decode",description="Decode bytes",parameters={type="object"},
    execute=function(args)
        local bytes = sai.encoding.decode(args.format, args.data)
        return {bytes=sai.json.array({string.byte(bytes, 1, #bytes)}), length=#bytes}
    end})
"#;

/// 【编码测试】【加载计费】初始化阶段同样限制原生字节计算，防止无 Lua 循环的大量调用绕过预算
/// @returns 无；超量初始化失败，小规模纯计算仍能用于预计算常量
#[test]
fn native_decoding_is_budgeted_during_initialization() {
    let mut package = package(
        r#"
        local text = string.rep("a", 1024)
        for i = 1, 8 do sai.encoding.decode("url", text) end
    "#,
    );
    package.manifest.limits.instructions = 4000;
    let result = PluginRuntime::load(
        package,
        json!({}),
        Capabilities::default(),
        Arc::new(RecordingHost::default()),
    );
    let error = result
        .err()
        .expect("unbounded native initialization succeeded");
    assert!(format!("{error:#}").contains("instruction budget"));
    runtime(
        r#"
        local text = sai.encoding.to_utf8(sai.encoding.decode("hex", "616263"))
        assert(text == "abc")
    "#,
    );
}

/// 【编码测试】【原始字节】三种编码保留 NUL 与非法 UTF-8，百分号解码保留加号和畸形转义
/// @returns 无；逐字节比较宿主解码结果
#[tokio::test]
async fn decoding_preserves_binary_data_and_url_semantics() {
    let plugin = runtime(DECODE_TOOL);
    for (format, data, expected) in [
        ("hex", "00fF01", vec![0, 255, 1]),
        ("base64", "AP8B", vec![0, 255, 1]),
        ("url", "%00%ff%01", vec![0, 255, 1]),
        ("url", "a+b%20c%zz%", b"a+b c%zz%".to_vec()),
        ("hex", "", vec![]),
        ("base64", "", vec![]),
        ("url", "", vec![]),
    ] {
        let output = plugin
            .call_tool(
                "decode",
                json!({"format":format,"data":data}),
                InvocationContext::default(),
            )
            .await
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&output).unwrap(),
            json!({"bytes":expected,"length":expected.len()})
        );
    }
}

/// 【编码测试】【严格校验】字节解码不隐式去空白、不接受错误编码或非字符串参数
/// @returns 无；所有非法参数必须在宿主边界失败
#[tokio::test]
async fn decoding_rejects_malformed_input_and_implicit_coercion() {
    let plugin = runtime(DECODE_TOOL);
    for (format, data) in [
        ("hex", "a"),
        ("hex", "gg"),
        ("hex", " 61 "),
        ("base64", "YQ"),
        ("base64", "YR=="),
        ("base64", " YQ== "),
        ("unknown", "abc"),
    ] {
        assert!(plugin
            .call_tool(
                "decode",
                json!({"format":format,"data":data}),
                InvocationContext::default()
            )
            .await
            .is_err());
    }
    for invalid in [Value::Null, json!(42), json!(true), json!([]), json!({})] {
        for args in [
            json!({"format":invalid,"data":"61"}),
            json!({"format":"hex","data":invalid}),
        ] {
            let error = plugin
                .call_tool("decode", args, InvocationContext::default())
                .await
                .unwrap_err();
            assert!(format!("{error:#}").contains("decode requires string format and input"));
        }
    }
    assert!(plugin
        .call_tool(
            "decode",
            json!({"format":"hex","data":"61"}),
            InvocationContext::default()
        )
        .await
        .is_ok());
}

/// 【编码测试】【文本转换】默认严格 UTF-8，可显式替换非法序列，转换选项只接受布尔值
/// @returns 无；转换失败后同一运行时仍可处理正常文本
#[tokio::test]
async fn utf8_conversion_distinguishes_strict_and_lossy_modes() {
    let plugin = runtime(
        r#"
        sai.register_tool({name="utf8",description="Decode UTF-8",parameters={type="object"},
            execute=function(args)
                local data = args.invalid_type and args.data or sai.encoding.decode("hex", args.data)
                if args.default then return sai.encoding.to_utf8(data) end
                return sai.encoding.to_utf8(data, args.lossy)
            end})
    "#,
    );
    for args in [
        json!({"data":"ff","default":true}),
        json!({"data":"ff","lossy":false}),
        json!({"data":"61","lossy":"true"}),
        json!({"data":42,"invalid_type":true,"lossy":false}),
    ] {
        assert!(plugin
            .call_tool("utf8", args, InvocationContext::default())
            .await
            .is_err());
    }
    for (args, expected) in [
        (json!({"data":"fffe00","lossy":true}), "��\0"),
        (json!({"data":"e4b8ade69687","default":true}), "中文"),
    ] {
        assert_eq!(
            plugin
                .call_tool("utf8", args, InvocationContext::default())
                .await
                .unwrap(),
            expected
        );
    }
}

/// 【编码测试】【转换边界】输入与 UTF-8 替换展开结果都有字节上限，错误不会泄漏到下一次调用
/// @returns 无；上限按字节计算且不允许公开限制表覆盖
#[tokio::test]
async fn encoding_limits_input_and_lossy_expansion() {
    let mut package = package(
        r#"
        sai.limits.output_bytes = math.huge
        sai.register_tool({name="bounded",description="Bounded conversion",parameters={type="object"},
            execute=function(args)
                local data = string.rep(args.lossy and string.char(255) or "a", args.count)
                if args.lossy then return #sai.encoding.to_utf8(data, true) end
                return #sai.encoding.decode("url", data)
            end})
    "#,
    );
    package.manifest.limits.output_bytes = 1024;
    let plugin = PluginRuntime::load(
        package,
        json!({}),
        Capabilities::default(),
        Arc::new(RecordingHost::default()),
    )
    .unwrap();
    for (args, stage) in [
        (json!({"count":1025,"lossy":false}), "input"),
        (json!({"count":342,"lossy":true}), "output"),
    ] {
        let error = plugin
            .call_tool("bounded", args, InvocationContext::default())
            .await
            .unwrap_err();
        assert!(
            format!("{error:#}").contains(&format!("encoding {stage} exceeds plugin size limit"))
        );
    }
    for (args, expected) in [
        (json!({"count":1024,"lossy":false}), "1024"),
        (json!({"count":341,"lossy":true}), "1023"),
    ] {
        assert_eq!(
            plugin
                .call_tool("bounded", args, InvocationContext::default())
                .await
                .unwrap(),
            expected
        );
    }
}
