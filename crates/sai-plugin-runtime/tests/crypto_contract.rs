mod common;

use common::{package, runtime, RecordingHost};
use sai_plugin_runtime::{Capabilities, InvocationContext, PluginRuntime};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;

const DIGEST_TOOL: &str = r#"
sai.register_tool({name="digest",description="Hash bytes",parameters={type="object"},
    execute=function(args) return sai.crypto.digest(args.algorithm, args.data) end})
"#;

/// 【摘要测试】【分块边界】跨越 64 KiB 的二进制输入与独立标准库参考结果一致
/// @returns 无；固定样本由 Python hashlib 与 zlib 生成，BLAKE3 对照原版单次摘要
#[tokio::test]
async fn incremental_digests_preserve_every_byte_across_chunk_boundaries() {
    let plugin = runtime(
        r#"
        local bytes = {}
        for value = 0, 255 do bytes[#bytes + 1] = string.char(value) end
        local data = table.concat(bytes):rep(256) .. string.char(0)
        sai.register_tool({name="digest",description="Chunk boundary",parameters={type="object"},
            execute=function(args) return sai.crypto.digest(args.algorithm, data) end})
    "#,
    );
    for (algorithm, expected) in [
        ("md5", "467a452953ded315733c37360360e507"),
        ("sha1", "e59142c546b1069d7b8dd556adeae6e40ec376e8"),
        ("sha224", "973f090530b3eaa09e18a6a4786caf9050beff5aab776de377bf9bc2"),
        ("sha256", "2deb0bd2129a9d3aed91e3cff58b3993752be549642890a3e853ec1065f9b617"),
        ("sha384", "a57179db64e65d12c2381273022ec2b3bea10fcb09b9f046d7be1d23f57765dba76f420675420e8d528ebfccf5da69f4"),
        ("sha512", "898102338e7423d0f0bdc2b404005d3bd55db3e5958e4a29ba1bd434a002d86b92fa40deafccaa062487134bd50c9df86bc16a07fa388986bdd316d2de3fe3e2"),
        ("sha3_224", "84514ad24d427c8e7e8e8545baa1dc1520512cc243bd1f32c2e6ee7a"),
        ("sha3_256", "2a579fff53e1d2a00b43d058a7b71bf260c961806cf535637556f667c35a1a6b"),
        ("sha3_384", "384e4a62ecd33f9666c1cad8f7c1884e7f0042c4ed895322801ce081bdb2f850f4dec345d1f9f4a8276d8365be3c989e"),
        ("sha3_512", "92cc4fffa653e40c229b1c5bc43efe319fb428807f3bae322a95719460c911dceccea1b0adf24d874e290ca1ccfdb4007c397c46acbcc0803b47bac5100f5a7a"),
        ("blake2b", "de858cbd4bba6114cc62da3360bea5629f728100989800fdb070d564bba290f19d29afe4b82a522d7c94822c43f51a61c214c92776c8064a23e38a9a7d03073e"),
        ("blake2s", "76333782043a7a23fb6cdcf48f73f32ea1634f3870597405fb5b31a17e80e82b"),
        ("crc32", "73626115"),
        ("adler32", "433b8772"),
    ] {
        let output = plugin.call_tool("digest", json!({"algorithm":algorithm}), InvocationContext::default()).await.unwrap();
        assert_eq!(output, expected, "{algorithm}");
    }
    let data = (0..65_537)
        .map(|value| (value % 256) as u8)
        .collect::<Vec<_>>();
    let output = plugin
        .call_tool(
            "digest",
            json!({"algorithm":"blake3"}),
            InvocationContext::default(),
        )
        .await
        .unwrap();
    assert_eq!(output, blake3::hash(&data).to_hex().to_string());
}

/// 【摘要测试】【标准向量】固定摘要验证空串、文本和原始二进制字节，零能力授权即可计算
/// @returns 无；任何算法结果变化都会导致测试失败
#[tokio::test]
async fn digests_match_standard_vectors_without_external_capabilities() {
    let plugin = runtime(DIGEST_TOOL);
    for (algorithm, data, expected) in [
        ("md5", "abc", "900150983cd24fb0d6963f7d28e17f72"),
        ("sha1", "abc", "a9993e364706816aba3e25717850c26c9cd0d89d"),
        (
            "sha256",
            "abc",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
        (
            "sha3_256",
            "abc",
            "3a985da74fe225b2045c172d6bd390bd855f086e3e9d525b46bfe24511431532",
        ),
        (
            "blake3",
            "abc",
            "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85",
        ),
        (
            "blake3",
            "",
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262",
        ),
        ("crc32", "123456789", "cbf43926"),
        ("adler32", "Wikipedia", "11e60398"),
        ("adler32", "", "00000001"),
    ] {
        assert_eq!(
            plugin
                .call_tool(
                    "digest",
                    json!({"algorithm":algorithm,"data":data}),
                    InvocationContext::default()
                )
                .await
                .unwrap(),
            expected,
            "{algorithm}"
        );
    }
    let plugin = runtime(
        r#"
        sai.register_tool({name="bytes",description="Raw bytes",parameters={type="object"},
            execute=function() return sai.crypto.digest("sha256", string.char(0, 255, 1)) end})
    "#,
    );
    assert_eq!(
        plugin
            .call_tool("bytes", json!({}), InvocationContext::default())
            .await
            .unwrap(),
        "47ffa3ea45a70b8a41c2c0825df323c00a8b7a01c1ea06083cc41dddcc001123"
    );
}

/// 【摘要测试】【参数类型】宿主拒绝隐式字符串转换和未知算法，错误后实例仍可复用
/// @returns 无；非法调用必须返回对应类型或算法错误
#[tokio::test]
async fn digest_rejects_non_strings_and_unknown_algorithms() {
    let plugin = runtime(DIGEST_TOOL);
    for invalid in [Value::Null, json!(42), json!(false), json!([]), json!({})] {
        for args in [
            json!({"algorithm":invalid,"data":"abc"}),
            json!({"algorithm":"sha256","data":invalid}),
        ] {
            let error = plugin
                .call_tool("digest", args, InvocationContext::default())
                .await
                .unwrap_err();
            assert!(format!("{error:#}").contains("digest requires string algorithm and bytes"));
        }
    }
    for algorithm in ["unknown", "SHA256", "b2sum", "sha3-256"] {
        let error = plugin
            .call_tool(
                "digest",
                json!({"algorithm":algorithm,"data":"abc"}),
                InvocationContext::default(),
            )
            .await
            .unwrap_err();
        assert!(format!("{error:#}").contains("unsupported digest algorithm"));
    }
    assert_eq!(
        plugin
            .call_tool(
                "digest",
                json!({"algorithm":"crc32","data":""}),
                InvocationContext::default()
            )
            .await
            .unwrap(),
        "00000000"
    );
}

/// 【摘要测试】【字节边界】摘要输入按字节限制，Lua 修改公开限制表不能扩大宿主预算
/// @returns 无；等于上限成功，超出上限失败且不污染后续调用
#[tokio::test]
async fn digest_input_limit_is_enforced_by_the_host() {
    let mut package = package(&format!(
        "sai.limits.output_bytes = math.huge\n{DIGEST_TOOL}"
    ));
    package.manifest.limits.output_bytes = 1024;
    let plugin = PluginRuntime::load(
        package,
        json!({}),
        Capabilities::default(),
        Arc::new(RecordingHost::default()),
    )
    .unwrap();
    for data in ["x".repeat(1025), "界".repeat(342)] {
        let error = plugin
            .call_tool(
                "digest",
                json!({"algorithm":"sha256","data":data}),
                InvocationContext::default(),
            )
            .await
            .unwrap_err();
        assert!(format!("{error:#}").contains("crypto input exceeds plugin size limit"));
    }
    let output = plugin
        .call_tool(
            "digest",
            json!({"algorithm":"sha256","data":"x".repeat(1024)}),
            InvocationContext::default(),
        )
        .await
        .unwrap();
    assert_eq!(output.len(), 64);
}

/// 【摘要测试】【宿主计费】原生摘要消耗共用指令预算，连续调用不能反复使用同一份额度
/// @returns 无；耗尽预算后失败，新调用重新获得预算
#[tokio::test]
async fn native_hash_work_consumes_the_invocation_budget_and_resets() {
    let mut package = package(
        r#"
        sai.register_tool({name="work",description="Native work budget",parameters={type="object"},
            execute=function()
                local data = string.rep("x", 1024)
                for i = 1, 8 do sai.crypto.digest("sha256", data) end
                return "unexpected"
            end})
        sai.register_tool({name="ping",description="Recovery",parameters={type="object"},
            execute=function() return sai.crypto.digest("adler32", "") end})
    "#,
    );
    package.manifest.limits.instructions = 4000;
    let plugin = PluginRuntime::load(
        package,
        json!({}),
        Capabilities::default(),
        Arc::new(RecordingHost::default()),
    )
    .unwrap();
    let error = plugin
        .call_tool("work", json!({}), InvocationContext::default())
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("instruction budget"));
    assert_eq!(
        plugin
            .call_tool("ping", json!({}), InvocationContext::default())
            .await
            .unwrap(),
        "00000001"
    );
}

/// 【摘要测试】【超时恢复】反复调用宿主摘要仍受回调截止时间约束，超时后释放虚拟机
/// @returns 无；超时错误与后续成功调用共同证明回收完成
#[tokio::test]
async fn digest_loops_time_out_and_release_the_vm() {
    let mut package = package(
        r#"
        sai.register_tool({name="work",description="Timed work",parameters={type="object"},
            execute=function() while true do sai.crypto.digest("sha3_512", "") end end})
        sai.register_tool({name="ping",description="Recovery",parameters={type="object"},execute=function() return "pong" end})
    "#,
    );
    package.manifest.limits.instructions = 20_000_000;
    package.manifest.limits.timeout_ms = 100;
    let plugin = PluginRuntime::load(
        package,
        json!({}),
        Capabilities::default(),
        Arc::new(RecordingHost::default()),
    )
    .unwrap();
    let error = plugin
        .call_tool("work", json!({}), InvocationContext::default())
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("timed out"), "{error:#}");
    let result = tokio::time::timeout(
        Duration::from_secs(3),
        plugin.call_tool("ping", json!({}), InvocationContext::default()),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(result, "pong");
}
