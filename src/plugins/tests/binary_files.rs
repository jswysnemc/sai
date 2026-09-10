use crate::paths::SaiPaths;
use crate::plugins::private::PrivatePluginHost;
use sai_plugin_runtime::{InvocationContext, PluginManifest, PluginPackage, PluginRuntime};
use serde_json::json;
use std::sync::Arc;

pub(super) const SOURCE: &str = r#"
    sai.register_tool({name='write',description='Write file',access='writes',parameters={type='object'},execute=function(args)
        if args.probe then return 'idle' end
        local data=sai.binary.decode_base64(args.data or 'aGVsbG8=')
        local result=data:write(args.path)
        data:close()
        return result
    end})
"#;

/// 【二进制文件测试】【真实运行时】通过实际私有宿主包装层执行受控写入。
/// @param root 临时应用目录；paths 为目录授权
/// @returns 使用真实文件系统的运行时
fn runtime(root: &std::path::Path, paths: &[&str]) -> PluginRuntime {
    let manifest=PluginManifest::parse(&json!({"api_version":1,"id":"binary-files","version":"1.0.0",
        "name":"Files","description":"Real binary writes","entry":"init.lua","capabilities":{"binary":{"write_paths":paths}}}).to_string()).unwrap();
    let grants = manifest.capabilities.clone();
    let package =
        PluginPackage::new(manifest, [("init.lua".into(), SOURCE.into())].into()).unwrap();
    PluginRuntime::load(
        package,
        json!({}),
        grants,
        Arc::new(PrivatePluginHost::new(
            &SaiPaths::for_tests(root),
            "binary-files",
        )),
    )
    .unwrap()
}

/// 【二进制文件测试】【可信目录】在临时工作区执行写入。
/// @param root 临时目录
/// @returns 可写上下文
pub(super) fn context(root: &std::path::Path) -> InvocationContext {
    InvocationContext {
        workdir: root.to_string_lossy().into_owned(),
        allow_writes: true,
        ..Default::default()
    }
}

/// 【二进制文件测试】【完整发布】缺失输出目录可创建，覆盖文件保持原子发布且不遗留暂存文件。
#[tokio::test]
async fn writes_create_authorized_roots_and_publish_complete_files() {
    let root = tempfile::tempdir().unwrap();
    let plugin = runtime(root.path(), &["output/nested"]);
    for encoded in ["aGVsbG8=", "bmV3"] {
        let result = plugin
            .call_tool(
                "write",
                json!({"path":"output/nested/a.png","data":encoded}),
                context(root.path()),
            )
            .await
            .unwrap();
        let result: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(
            result["path"],
            root.path()
                .join("output/nested/a.png")
                .to_string_lossy()
                .as_ref()
        );
    }
    assert_eq!(
        std::fs::read(root.path().join("output/nested/a.png")).unwrap(),
        b"new"
    );
    assert_eq!(
        std::fs::read_dir(root.path().join("output/nested"))
            .unwrap()
            .count(),
        1
    );
}

/// 【二进制文件测试】【先授权后创建】越界、父目录跳转及设备名称不能产生任何目录。
#[tokio::test]
async fn rejected_paths_do_not_create_parents_or_touch_existing_files() {
    let root = tempfile::tempdir().unwrap();
    let plugin = runtime(root.path(), &["allowed"]);
    for path in [
        "outside/nested/file",
        "allowed/../outside/file",
        "allowed/con.png",
        "allowed/a:stream",
        "allowed/trailing./file",
    ] {
        assert!(
            plugin
                .call_tool("write", json!({"path":path}), context(root.path()))
                .await
                .is_err(),
            "{path}"
        );
    }
    assert!(!root.path().join("outside").exists());
    assert!(!root.path().join("allowed").exists());
}

/// 【二进制文件测试】【目录拒绝】不能用文件内容替换目录。
#[tokio::test]
async fn output_directories_are_not_replaced_by_files() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("allowed/directory")).unwrap();
    let plugin = runtime(root.path(), &["allowed"]);
    assert!(plugin
        .call_tool(
            "write",
            json!({"path":"allowed/directory"}),
            context(root.path())
        )
        .await
        .is_err());
    assert!(root.path().join("allowed/directory").is_dir());
}

/// 【二进制文件测试】【链接越界】目录链接和末级链接都不能写入授权根之外。
#[cfg(unix)]
#[tokio::test]
async fn symbolic_links_cannot_escape_output_roots() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("allowed")).unwrap();
    std::fs::write(outside.path().join("keep"), b"original").unwrap();
    std::os::unix::fs::symlink(outside.path(), root.path().join("allowed/escape")).unwrap();
    std::os::unix::fs::symlink(
        outside.path().join("keep"),
        root.path().join("allowed/file"),
    )
    .unwrap();
    let plugin = runtime(root.path(), &["allowed"]);
    for path in ["allowed/escape/new/nested/file", "allowed/file"] {
        assert!(plugin
            .call_tool("write", json!({"path":path}), context(root.path()))
            .await
            .is_err());
    }
    assert!(!outside.path().join("new").exists());
    assert_eq!(
        std::fs::read(outside.path().join("keep")).unwrap(),
        b"original"
    );
}
