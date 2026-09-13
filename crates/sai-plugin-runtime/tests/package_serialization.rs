mod common;

use sai_plugin_runtime::{PluginManifest, PluginPackage};
use serde_json::json;
use std::path::Path;

const MANIFEST_LIMIT: usize = 64 * 1024;

/// 【清单序列化测试】【边界输入】构造读取合法但规范输出接近上限的真实清单
/// @param defaults_overflow 是否省略缺省值，使补全后的紧凑 JSON 也超过上限
/// @returns 不超过文件读取上限且字段本身均合法的紧凑清单
fn boundary_manifest(defaults_overflow: bool) -> String {
    let mut value = serde_json::to_value(common::manifest()).unwrap();
    if defaults_overflow {
        value.as_object_mut().unwrap().remove("limits");
    }
    for width in 3900..4070 {
        let paths = (0..16)
            .map(|index| format!("notes/{index}/{}", "a".repeat(width)))
            .collect::<Vec<_>>();
        value["capabilities"] = json!({"system":{"read_paths":paths}});
        let input = serde_json::to_string(&value).unwrap();
        let manifest = PluginManifest::parse(&input).unwrap();
        let compact = serde_json::to_vec(&manifest).unwrap().len() + 1;
        let pretty = serde_json::to_vec_pretty(&manifest).unwrap().len() + 1;
        if input.len() <= MANIFEST_LIMIT
            && if defaults_overflow {
                compact > MANIFEST_LIMIT
            } else {
                compact <= MANIFEST_LIMIT && pretty > MANIFEST_LIMIT
            }
        {
            return input;
        }
    }
    panic!("manifest boundary fixture was not found");
}

/// 【清单序列化测试】【实际读取】把输入写入真实源码目录并经过正式加载器
/// @param root 临时源码目录；manifest 为输入清单
/// @returns 通过文件和结构校验的快照
fn package(root: &Path, manifest: &str) -> PluginPackage {
    std::fs::write(root.join("sai-plugin.json"), manifest).unwrap();
    std::fs::write(root.join("init.lua"), "return true\n").unwrap();
    PluginPackage::from_directory(root).unwrap()
}

/// 【清单序列化测试】【普通输出】小清单保持可读排版和末尾换行
/// @returns 无，保存后仍能读取同一清单与源码
#[test]
fn ordinary_manifest_serialization_remains_readable_and_reloadable() {
    let root = tempfile::tempdir().unwrap();
    let original = package(
        root.path(),
        &serde_json::to_string(&common::manifest()).unwrap(),
    );
    let bytes = original.manifest_json().unwrap();
    assert!(bytes.starts_with(b"{\n  "));
    assert!(bytes.ends_with(b"\n"));
    std::fs::write(root.path().join("sai-plugin.json"), bytes).unwrap();
    let loaded = PluginPackage::from_directory(root.path()).unwrap();
    assert_eq!(original.sources(), loaded.sources());
    assert_eq!(
        serde_json::to_value(original.manifest).unwrap(),
        serde_json::to_value(loaded.manifest).unwrap()
    );
}

/// 【清单序列化测试】【格式膨胀】合法输入不能因保存时增加缩进而变成不可加载的包
/// @returns 无，较大清单使用紧凑编码，字段与有效权限不变
#[test]
fn large_manifest_falls_back_to_compact_json_and_round_trips() {
    let root = tempfile::tempdir().unwrap();
    let original = package(root.path(), &boundary_manifest(false));
    let bytes = original.manifest_json().unwrap();
    assert!(bytes.len() <= MANIFEST_LIMIT);
    assert!(!bytes.starts_with(b"{\n"));
    std::fs::write(root.path().join("sai-plugin.json"), bytes).unwrap();
    let loaded = PluginPackage::from_directory(root.path()).unwrap();
    assert_eq!(
        serde_json::to_value(original.manifest).unwrap(),
        serde_json::to_value(loaded.manifest).unwrap()
    );
}

/// 【清单序列化测试】【缺省膨胀】补全字段后仍超限时明确失败，不生成不可加载的输出
/// @returns 无，原始清单仍然有效且未修改
#[test]
fn default_expansion_beyond_the_manifest_limit_fails_before_writing() {
    let root = tempfile::tempdir().unwrap();
    let input = boundary_manifest(true);
    let original = package(root.path(), &input);
    assert!(format!("{:#}", original.manifest_json().unwrap_err()).contains("64 KiB"));
    assert_eq!(
        std::fs::read_to_string(root.path().join("sai-plugin.json")).unwrap(),
        input
    );
}
