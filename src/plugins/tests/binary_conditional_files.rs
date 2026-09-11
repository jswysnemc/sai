use super::binary_conditional_support::{configured, context, digest, runtime};
use crate::paths::SaiPaths;
use crate::plugins::private::PrivatePluginHost;
use sai_plugin_runtime::{InvocationContext, PluginManifest, PluginPackage, PluginRuntime};
use serde_json::json;
use std::sync::Arc;

/// 【条件写入测试】【缺失文件创建】仅当目标不存在时创建完整文件，重复创建不覆盖原内容
/// @returns 无；真实私有宿主提供条件发布能力
#[tokio::test]
async fn conditional_writes_create_missing_files_without_overwriting_existing_bytes() {
    let root = tempfile::tempdir().unwrap();
    let manifest = PluginManifest::parse(
        &json!({
            "api_version":1,"id":"conditional-files","version":"1.0.0","name":"Conditional files",
            "description":"Real conditional writes","entry":"init.lua",
            "capabilities":{"system":{"read_paths":["output"]},"binary":{"write_paths":["output"]}},
        })
        .to_string(),
    )
    .unwrap();
    let grants = manifest.capabilities.clone();
    let source = r#"
        --- 【条件写入测试】【真实创建】相同创建条件只能成功一次
        --- @return table 两次比较交换结果
        local function run()
            local first=sai.binary.decode_base64("e30=")
            local second=sai.binary.decode_base64("bmV3")
            return {first=first:write_if("output/index.json",nil),second=second:write_if("output/index.json",nil)}
        end
        sai.register_tool({name="run",description="Conditional create",access="writes",parameters={type="object"},execute=run})
    "#;
    let package =
        PluginPackage::new(manifest, [("init.lua".into(), source.into())].into()).unwrap();
    let plugin = PluginRuntime::load(
        package,
        json!({}),
        grants,
        Arc::new(PrivatePluginHost::new(
            &SaiPaths::for_tests(root.path()),
            "conditional-files",
        )),
    )
    .unwrap();
    let context = InvocationContext {
        workdir: root.path().display().to_string(),
        allow_writes: true,
        ..Default::default()
    };
    let output = plugin.call_tool("run", json!({}), context).await.unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&output).unwrap(),
        json!({"first":true,"second":false})
    );
    assert_eq!(
        std::fs::read(root.path().join("output/index.json")).unwrap(),
        b"{}"
    );
}

/// 【条件文件测试】【摘要替换】旧摘要只允许一次完整替换，摘要大小写不改变语义
/// @returns 无；冲突不修改文件，不遗留暂存内容
#[tokio::test]
async fn matching_full_digest_replaces_once_and_conflicts_leave_bytes_unchanged() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("output")).unwrap();
    let path = root.path().join("output/index.json");
    let old = b"\0\xfforiginal";
    std::fs::write(&path, old).unwrap();
    let plugin = runtime(root.path(), &["output"], &["output"]);
    for (expected, wanted) in [
        (digest(old).to_uppercase(), "true"),
        (digest(old), "false"),
        (digest(b"other"), "false"),
    ] {
        assert_eq!(
            plugin
                .call_tool(
                    "run",
                    json!({"path":"output/index.json","expected":expected,"data":"replaced"}),
                    context(root.path())
                )
                .await
                .unwrap(),
            wanted
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"replaced");
    }
    assert_eq!(
        std::fs::read_dir(root.path().join("output"))
            .unwrap()
            .count(),
        1
    );
}

/// 【条件文件测试】【缺失与空文件】不存在和空普通文件拥有不同修订条件
/// @returns 无；缺失摘要条件不创建父目录，空文件可以通过真实摘要替换
#[tokio::test]
async fn missing_targets_do_not_match_empty_file_digests_or_create_parent_directories() {
    let root = tempfile::tempdir().unwrap();
    let plugin = runtime(root.path(), &["output"], &["output"]);
    assert_eq!(
        plugin
            .call_tool(
                "run",
                json!({"path":"output/nested/index.json","expected":digest(b"")}),
                context(root.path())
            )
            .await
            .unwrap(),
        "false"
    );
    assert!(!root.path().join("output").exists());
    assert_eq!(
        plugin
            .call_tool(
                "run",
                json!({"path":"output/nested/index.json","data":""}),
                context(root.path())
            )
            .await
            .unwrap(),
        "true"
    );
    assert_eq!(
        plugin
            .call_tool(
                "run",
                json!({"path":"output/nested/index.json"}),
                context(root.path())
            )
            .await
            .unwrap(),
        "false"
    );
    assert_eq!(
        std::fs::read(root.path().join("output/nested/index.json")).unwrap(),
        b""
    );
    assert_eq!(
        plugin
            .call_tool(
                "run",
                json!({"path":"output/nested/index.json","expected":digest(b"")}),
                context(root.path())
            )
            .await
            .unwrap(),
        "true"
    );
}

/// 【条件文件测试】【比较大小限制】旧文件完整摘要受 VM 二进制上限约束，超限不截断也不替换
/// @returns 无；缩小旧文件后同一实例仍可正常比较和写入
#[tokio::test]
async fn oversized_existing_files_are_rejected_without_partial_comparison() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("output")).unwrap();
    let path = root.path().join("output/a");
    let old = vec![7; 1025];
    std::fs::write(&path, &old).unwrap();
    let plugin = configured(root.path(), &["output"], &["output"], |manifest| {
        manifest.limits.binary_bytes = 1024
    });
    let error = plugin
        .call_tool(
            "run",
            json!({"path":"output/a","expected":digest(&old)}),
            context(root.path()),
        )
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("size limit"), "{error:#}");
    assert_eq!(std::fs::read(&path).unwrap(), old);
    assert_eq!(
        plugin
            .call_tool("run", json!({"path":"output/a"}), context(root.path()))
            .await
            .unwrap(),
        "false"
    );
    std::fs::write(&path, &old[..1024]).unwrap();
    assert_eq!(
        plugin
            .call_tool(
                "run",
                json!({"path":"output/a","expected":digest(&old[..1024])}),
                context(root.path())
            )
            .await
            .unwrap(),
        "true"
    );
}

/// 【条件文件测试】【并发修订】不同插件实例共用正式宿主锁，相同旧摘要只能发布一个结果
/// @returns 无；全部参与者中恰好一次成功，磁盘内容来自成功结果
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn competing_hosts_cannot_both_replace_the_same_revision() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("output")).unwrap();
    let path = root.path().join("output/index.json");
    std::fs::write(&path, b"initial").unwrap();
    let mut calls = tokio::task::JoinSet::new();
    for index in 0..8 {
        let plugin = runtime(root.path(), &["output"], &["output"]);
        let invocation = context(root.path());
        calls.spawn(async move { (index,plugin.call_tool("run",json!({"path":"output/index.json","data":index.to_string(),"expected":digest(b"initial")}),invocation).await) });
    }
    let mut winners = Vec::new();
    while let Some(result) = calls.join_next().await {
        let (index, result) = result.unwrap();
        match result.unwrap().as_str() {
            "true" => winners.push(index),
            "false" => {}
            value => panic!("unexpected result: {value}"),
        }
    }
    assert_eq!(winners.len(), 1);
    assert_eq!(
        std::fs::read_to_string(path).unwrap(),
        winners[0].to_string()
    );
}
