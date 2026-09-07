mod common;

use common::manifest;
use sai_plugin_runtime::{PluginManifest, PluginPackage};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// 【插件测试】【清单契约】拒绝不支持版本、跨平台路径和无效网络能力。
#[test]
fn rejects_invalid_versions_paths_and_capabilities() {
    let baseline = serde_json::to_value(manifest()).unwrap();
    for (field, value) in [
        ("api_version", json!(2)),
        ("id", json!("../escape")),
        ("version", json!("latest")),
        ("entry", json!("../outside.lua")),
        ("entry", json!("/tmp/file.lua")),
        ("entry", json!("C:\\file.lua")),
        ("entry", json!("module.so")),
        ("id", json!("aux")),
        ("entry", json!("NUL.lua")),
        ("entry", json!("invalid?/init.lua")),
        (
            "capabilities",
            json!({"http":["https://example.test/path"]}),
        ),
        (
            "capabilities",
            json!({"http":["https://user:secret@example.test"]}),
        ),
        ("capabilities", json!({"http":["file:///"]})),
        ("limits", json!({"instructions":u64::MAX})),
    ] {
        let mut value_to_check = baseline.clone();
        value_to_check[field] = value;
        assert!(
            PluginManifest::parse(&value_to_check.to_string()).is_err(),
            "accepted {value_to_check}"
        );
    }
    let mut unknown: Value = baseline;
    unknown["ignored_typo"] = json!(true);
    assert!(PluginManifest::parse(&unknown.to_string()).is_err());
}

/// 【插件测试】【源码契约】缺失入口或越界源码名不得建立运行快照。
#[test]
fn source_packages_require_valid_entry_and_relative_paths() {
    assert!(PluginPackage::new(manifest(), BTreeMap::new()).is_err());
    let sources = BTreeMap::from([
        ("init.lua".into(), String::new()),
        ("../other.lua".into(), String::new()),
    ]);
    assert!(PluginPackage::new(manifest(), sources).is_err());
}

/// 【插件测试】【目录隔离】插件源码不能通过符号链接读取包外文件。
#[cfg(unix)]
#[test]
fn directory_packages_reject_symbolic_links() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(
        root.path().join("sai-plugin.json"),
        serde_json::to_vec(&manifest()).unwrap(),
    )
    .unwrap();
    let external = tempfile::NamedTempFile::new().unwrap();
    std::os::unix::fs::symlink(external.path(), root.path().join("init.lua")).unwrap();
    assert!(PluginPackage::from_directory(root.path()).is_err());
}
