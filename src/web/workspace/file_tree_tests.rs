use super::read_tree;

/// 【工作区】【文件树】隐藏开关统一控制根目录、子目录与按需展开目录
/// 参数: 无；返回: 无，隐藏条目显示状态不一致时断言失败
#[test]
fn hidden_files_follow_visibility_at_every_depth() {
    let workspace = tempfile::tempdir().unwrap();
    let root = workspace.path();
    for directory in ["src", ".config", ".git", "node_modules"] {
        std::fs::create_dir(root.join(directory)).unwrap();
    }
    for file in [
        "README.md",
        ".env",
        "src/main.rs",
        "src/.local",
        ".config/settings.json",
    ] {
        std::fs::write(root.join(file), "").unwrap();
    }

    let hidden = read_tree(root, "", 3, false).unwrap();
    assert_eq!(
        hidden
            .iter()
            .map(|node| node.name.as_str())
            .collect::<Vec<_>>(),
        ["src", "README.md"]
    );
    assert_eq!(hidden[0].children.len(), 1);
    assert_eq!(hidden[0].children[0].name, "main.rs");

    let shown = read_tree(root, "", 3, true).unwrap();
    for name in [".env", ".config", ".git", "src", "README.md"] {
        assert!(
            shown.iter().any(|node| node.name == name),
            "缺少条目 {name}"
        );
    }
    assert!(!shown.iter().any(|node| node.name == "node_modules"));
    assert_eq!(
        shown
            .iter()
            .find(|node| node.name == ".config")
            .unwrap()
            .children[0]
            .path,
        ".config/settings.json"
    );

    let shallow = read_tree(root, "", 1, true).unwrap();
    assert!(shallow.iter().all(|node| node.children.is_empty()));
    for show_hidden in [false, true] {
        let children = read_tree(root, "src", 2, show_hidden).unwrap();
        assert_eq!(
            children.iter().any(|node| node.name == ".local"),
            show_hidden
        );
        assert!(children.iter().any(|node| node.path == "src/main.rs"));
    }
}
