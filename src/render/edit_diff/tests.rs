use super::renderer::render_for_test;
use serde_json::json;

#[test]
fn repl_diff_keeps_symmetric_background_insets() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("sample.txt");
    std::fs::write(&path, "old\n").unwrap();
    let args = json!({
        "path": path.display().to_string(),
        "old_string": "old",
        "new_string": "new"
    })
    .to_string();

    let output = render_for_test(&args).unwrap();

    assert!(output.contains("\x1b[K"));
    assert!(output.contains("\x1b[2D\x1b[3X"));
}