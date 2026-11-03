//! 补丁工具从调用预览到执行结果的终端渲染回归测试。

use super::{ToolCallDisplayMode, edit_diff, tool_view};
use serde_json::json;

/// 【文件补丁测试】【终端集成】预览显示多个文件与删除内容，结果到达后使用真实差异和计数。
/// 参数: 无；返回: 无。
#[tokio::test]
async fn apply_patch_previews_and_results_use_the_edit_renderer() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("old"), "removed-line\n").unwrap();
    crate::runtime_cwd::scope(root.path().into(), async {
        let patch =
            "*** Begin Patch\n*** Add File: new\n+added-line\n*** Delete File: old\n*** End Patch";
        let args = json!({"patch":patch}).to_string();
        let preview = edit_diff::preview_from_arguments(&args).unwrap();
        assert_eq!(preview.line_counts(), (1, 1));
        let rendered = edit_diff::render_patch_preview_for_transcript(&preview);
        assert!(rendered.contains("added-line") && rendered.contains("removed-line"));
        assert_eq!(crate::llm::streamed_diff_counts(&args), Some((1, 0)));
        let mut view = tool_view::ToolView::running("apply_patch".into(), args.clone());
        let mut registry = crate::tools::ToolRegistry::new();
        crate::tools::apply_patch::register(&mut registry);
        view.finish(true, registry.call("apply_patch", &args).await.unwrap());
        let rendered = tool_view::render(&view, ToolCallDisplayMode::Full);
        assert!(
            rendered.contains("Patched") && rendered.contains("added-line"),
            "{rendered}"
        );
        assert_eq!(rendered.matches("added-line").count(), 1, "{rendered}");
        assert_eq!(rendered.matches("removed-line").count(), 1, "{rendered}");
        assert_eq!(
            super::guide_symbols::tool("apply_patch"),
            super::guide_symbols::tool("str_replace")
        );
        view.finish(false, "tool error: file edit conflict".into());
        assert!(tool_view::render(&view, ToolCallDisplayMode::Full).contains("file edit conflict"));
    })
    .await;
}
