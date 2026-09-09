use super::*;

/// 【文件授权测试】【根目录竞态】校验后的目录被换成符号链接时，打开锚点必须失败。
#[test]
fn replacing_a_resolved_anchor_with_a_symlink_cannot_change_its_authority() {
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("allowed");
    let outside = root.path().join("outside");
    std::fs::create_dir(&original).unwrap();
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("secret"), "outside").unwrap();
    let resolved = std::fs::canonicalize(&original).unwrap();
    std::fs::rename(&original, root.path().join("renamed")).unwrap();
    std::os::unix::fs::symlink(&outside, &original).unwrap();
    assert!(open_anchor(&resolved).is_err());
}

/// 【文件授权测试】【句柄归属】打开后的目录即使被重命名并替换，读取仍指向原授权目录。
#[test]
fn an_open_anchor_keeps_the_original_directory_after_replacement() {
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("allowed");
    let outside = root.path().join("outside");
    std::fs::create_dir(&original).unwrap();
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(original.join("file"), "inside").unwrap();
    std::fs::write(outside.join("file"), "outside").unwrap();
    let anchor = open_anchor(&std::fs::canonicalize(&original).unwrap()).unwrap();
    std::fs::rename(&original, root.path().join("renamed")).unwrap();
    std::os::unix::fs::symlink(&outside, &original).unwrap();
    assert_eq!(anchor.read_to_string("file").unwrap(), "inside");
}

/// 【文件授权测试】【末级竞态】单文件授权的目标换成同级目录链接后，不能枚举未授权目录。
#[test]
fn a_single_file_grant_cannot_follow_a_replacement_link_to_a_sibling_directory() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("allowed"), "file").unwrap();
    std::fs::create_dir(root.path().join("private")).unwrap();
    std::fs::write(root.path().join("private/secret"), "private").unwrap();
    let capabilities: Capabilities =
        serde_json::from_value(serde_json::json!({"system":{"read_paths":["allowed"]}})).unwrap();
    let context = SystemContext {
        workdir: root.path().to_string_lossy().into_owned(),
        allow_writes: false,
    };
    let authorized = authorize("allowed", &context, &capabilities).unwrap();
    std::fs::remove_file(root.path().join("allowed")).unwrap();
    std::os::unix::fs::symlink("private", root.path().join("allowed")).unwrap();
    assert!(authorized.open_directory().is_err());
}
