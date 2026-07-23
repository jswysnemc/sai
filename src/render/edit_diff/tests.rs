use super::renderer::render_for_test;
use serde_json::json;

#[test]
fn renders_patch_update_as_edited() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("sample.rs");
    std::fs::write(&path, "fn main() {\n    old();\n}\n").unwrap();
    let args = json!({
        "patch": format!(
            "*** Begin Patch\n*** Update File: {}\n@@\n fn main() {{\n-    old();\n+    new();\n }}\n*** End Patch",
            path.display()
        )
    })
    .to_string();

    let output = render_for_test(&args).unwrap();
    let plain = strip_ansi_for_test(&output);

    assert!(plain.contains("Edited"));
    assert!(plain.contains("(+1 -1)"));
    assert!(plain.contains("  2 -      old();"));
    assert!(plain.contains("  2 +      new();"));
}

#[test]
fn repl_diff_keeps_terminal_erase_to_line_end() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("sample.txt");
    std::fs::write(&path, "old\n").unwrap();
    let args = json!({
        "patch": format!(
            "*** Begin Patch\n*** Update File: {}\n@@\n-old\n+new\n*** End Patch",
            path.display()
        )
    })
    .to_string();

    let output = render_for_test(&args).unwrap();

    assert!(output.contains("\x1b[K"));
}

#[test]
fn renders_partial_json_patch_when_patch_string_is_closed() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("sample.txt");
    std::fs::write(&path, "old\n").unwrap();
    let patch = format!(
        "*** Begin Patch\n*** Update File: {}\n@@\n-old\n+new\n*** End Patch",
        path.display()
    );
    let patch_json = serde_json::to_string(&patch).unwrap();
    let partial = format!(r#"{{"patch":{patch_json},"path":""#);

    let output = render_for_test(&partial).unwrap();
    let plain = strip_ansi_for_test(&output);

    assert!(plain.contains("Edited"));
    assert!(plain.contains("  1 -  old"));
    assert!(plain.contains("  1 +  new"));
}

#[test]
fn renders_patch_add_as_added() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("new.txt");
    let args = json!({
        "patch": format!(
            "*** Begin Patch\n*** Add File: {}\n+hello\n+world\n*** End Patch",
            path.display()
        )
    })
    .to_string();

    let output = render_for_test(&args).unwrap();
    let plain = strip_ansi_for_test(&output);

    assert!(plain.contains("Added"));
    assert!(plain.contains("(+2 -0)"));
    assert!(plain.contains("  1 +  hello"));
    assert!(plain.contains("  2 +  world"));
}

#[test]
fn renders_patch_delete_as_deleted() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("old.txt");
    std::fs::write(&path, "old\ntext\n").unwrap();
    let args = json!({
        "patch": format!(
            "*** Begin Patch\n*** Delete File: {}\n*** End Patch",
            path.display()
        )
    })
    .to_string();

    let output = render_for_test(&args).unwrap();
    let plain = strip_ansi_for_test(&output);

    assert!(plain.contains("Deleted"));
    assert!(plain.contains("(+0 -2)"));
    assert!(plain.contains("  1 -  old"));
    assert!(plain.contains("  2 -  text"));
}

/// 去除 ANSI 转义序列，方便断言可见文本。
///
/// 参数:
/// - `text`: 原始终端文本
///
/// 返回:
/// - 去除样式后的文本
fn strip_ansi_for_test(text: &str) -> String {
    let mut output = String::new();
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            output.push(ch);
        }
    }
    output
}
