use sai_plugin_runtime::Capabilities;
use serde_json::{json, Value};

/// 【删除授权测试】【声明解析】把独立文件授权交给正式清单能力解析器
/// @param system 系统能力的 JSON 声明
/// @returns 解析并验证后的能力集合
fn capabilities(system: Value) -> Capabilities {
    let capabilities: Capabilities = serde_json::from_value(json!({"system":system})).unwrap();
    capabilities.validate().unwrap();
    capabilities
}

/// 【删除授权测试】【独立交集】回收站、永久删除及旧读取写入能力不能互相授予
/// @returns 无；只保留声明与授权中同名、同路径的集合
#[test]
fn removal_and_trash_paths_are_independent_explicit_capabilities() {
    let declared =
        capabilities(json!({"remove_paths":["output","other"],"trash_paths":["output"]}));
    let removed = capabilities(json!({"remove_paths":["output"]}));
    let trashed = capabilities(json!({"trash_paths":["output"]}));
    let legacy: Capabilities = serde_json::from_value(json!({
        "system":{"read_paths":["output"]},"binary":{"write_paths":["output"]}
    }))
    .unwrap();
    assert_eq!(declared.intersection(&removed), removed);
    assert_eq!(declared.intersection(&trashed), trashed);
    assert_eq!(removed.intersection(&trashed), Capabilities::default());
    assert_eq!(declared.intersection(&legacy), Capabilities::default());
    assert!(removed.is_subset(&declared));
    assert!(trashed.is_subset(&declared));
    assert!(!removed.is_subset(&trashed));
    assert!(!declared.system.is_empty());
    assert_eq!(
        serde_json::to_value(&trashed).unwrap()["system"],
        json!({"trash_paths":["output"]})
    );
}

/// 【删除授权测试】【路径边界】每类授权独立限制数量，并沿用目录声明语法
/// @returns 无；越界数量、遍历路径和控制字符均拒绝
#[test]
fn removal_path_categories_validate_counts_and_path_syntax() {
    for name in ["remove_paths", "trash_paths"] {
        for path in [
            "",
            "../outside",
            "output/*.png",
            "output/a\0b",
            "~someone/files",
        ] {
            let parsed: Capabilities =
                serde_json::from_value(json!({"system":{name:[path]}})).unwrap();
            assert!(parsed.validate().is_err(), "{name}: {path:?}");
        }
        let paths: Vec<String> = (0..65).map(|index| format!("output/{index}")).collect();
        let parsed: Capabilities = serde_json::from_value(json!({"system":{name:paths}})).unwrap();
        assert!(parsed.validate().is_err());
        capabilities(json!({name:[".","~/pictures","output~1","/tmp/images"]}));
    }
}

/// 【删除授权测试】【兼容缺省】旧清单不包含新权限，路径变化会撤销旧授权
/// @returns 无；新增能力缺省关闭且不会沿用不一致目录
#[test]
fn old_manifests_and_changed_paths_cannot_inherit_removal_grants() {
    assert!(Capabilities::default().system.is_empty());
    assert!(serde_json::to_value(Capabilities::default())
        .unwrap()
        .get("system")
        .is_none());
    let before = capabilities(json!({"remove_paths":["output"],"trash_paths":["output"]}));
    let after = capabilities(json!({"remove_paths":["other"],"trash_paths":["other"]}));
    assert_eq!(before.intersection(&after), Capabilities::default());
    assert!(!before.is_subset(&after));
}
