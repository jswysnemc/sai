mod common;

use sai_plugin_runtime::PluginPackage;
use std::collections::BTreeMap;
use std::path::Path;

/// 【源码路径测试】【基础目录】建立包含清单和入口的真实源码目录
/// @param root 临时包目录，允许尚未创建
/// @returns 无，不读取用户配置或执行 Lua
fn write_package(root: &Path) {
    std::fs::create_dir_all(root).unwrap();
    std::fs::write(
        root.join("sai-plugin.json"),
        serde_json::to_vec(&common::manifest()).unwrap(),
    )
    .unwrap();
    std::fs::write(root.join("init.lua"), "return true\n").unwrap();
}

/// 【源码路径测试】【原生目录】保留真实目录层级和 UTF-8 名称，不限制包外父目录名称
/// @returns 无，Unix 根目录中的反斜杠不属于包内源码名
#[test]
fn native_nested_paths_preserve_each_source_and_unicode_name() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join(if cfg!(unix) {
        r"source \ directory"
    } else {
        "source directory"
    });
    write_package(&root);
    let nested = root.join("modules").join("辅助");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(nested.join("value.lua"), "return 'nested'\n").unwrap();
    std::fs::write(root.join("modules/value.lua"), "return 'parent'\n").unwrap();
    let package = PluginPackage::from_directory(&root).unwrap();
    assert_eq!(
        package.sources(),
        &BTreeMap::from([
            ("init.lua".into(), "return true\n".into()),
            ("modules/value.lua".into(), "return 'parent'\n".into()),
            ("modules/辅助/value.lua".into(), "return 'nested'\n".into()),
        ])
    );
}

/// 【源码路径测试】【非法名称】文件或目录名中的反斜杠不能解释为新的目录层级
/// @returns 无，即使没有同名目标，也拒绝更改源码路径含义
#[cfg(unix)]
#[test]
fn literal_backslashes_in_source_components_are_rejected() {
    for name in [r"modules\value.lua", r"modules\nested/value.lua"] {
        let root = tempfile::tempdir().unwrap();
        write_package(root.path());
        let path = root.path().join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "return 'literal name'\n").unwrap();
        let error = PluginPackage::from_directory(root.path()).unwrap_err();
        let message = format!("{error:#}");
        assert!(message.contains("invalid plugin source path"), "{message}");
        assert!(message.contains("normalized relative path"), "{message}");
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "return 'literal name'\n"
        );
    }
}

/// 【源码路径测试】【冲突拒绝】拒绝会在旧转换中覆盖另一份源码的路径，保持原文件
/// @returns 无，创建顺序不影响整包拒绝结果
#[cfg(unix)]
#[test]
fn path_aliases_cannot_replace_another_source_in_the_snapshot() {
    for alias_first in [false, true] {
        let root = tempfile::tempdir().unwrap();
        write_package(root.path());
        std::fs::create_dir(root.path().join("modules")).unwrap();
        let canonical = root.path().join("modules/value.lua");
        let alias = root.path().join(r"modules\value.lua");
        let mut files = [
            (&canonical, "return 'canonical'\n"),
            (&alias, "return 'alias'\n"),
        ];
        if alias_first {
            files.reverse();
        }
        for (path, content) in files {
            std::fs::write(path, content).unwrap();
        }
        assert!(PluginPackage::from_directory(root.path()).is_err());
        for (path, content) in files {
            assert_eq!(std::fs::read_to_string(path).unwrap(), content);
        }
    }
}
