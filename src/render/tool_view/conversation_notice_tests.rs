use super::{render, ConversationNoticeKind, ToolView};
use crate::render::{activity_animation::strip_ansi_for_test, render_expand, ToolCallDisplayMode};

/// 【终端】【会话通知测试】构造带类型和图片来源的完成通知工具。
/// 参数: `kind` 为来源，`text` 为正文，`sources` 为图片来源；返回: 完成的通知工具。
fn notice(kind: ConversationNoticeKind, text: &str, sources: &[String]) -> ToolView {
    let mut view = ToolView::running(
        "conversation_notice".into(),
        serde_json::json!({"kind":kind.as_str(),"image_sources":sources}).to_string(),
    );
    view.finish(true, text.into());
    view
}

/// 【终端】【续轮通知测试】续轮显示目标正文与英文标题，不出现内部中文提示。
/// 参数: 无；返回: 无。
#[test]
fn goal_notice_shows_the_objective_without_internal_prompt_labels() {
    let view = notice(
        ConversationNoticeKind::GoalContinuation,
        "Goal 自动续轮：检查读取布局",
        &[],
    );
    let full = strip_ansi_for_test(&render(&view, ToolCallDisplayMode::Full));
    assert!(full.contains("Continuing goal"), "{full}");
    assert!(full.contains("Goal: 检查读取布局"), "{full}");
    assert!(!full.contains("自动续轮"), "{full}");
    assert_eq!(full.matches("检查读取布局").count(), 1);
    assert!(render(&view, ToolCallDisplayMode::Hidden).is_empty());
}

/// 【终端】【排队通知测试】消息正文保留原文和换行，不当成结构化工具数据重新解析。
/// 参数: 无；返回: 无。
#[test]
fn queued_notice_preserves_literal_content_and_aligns_continuations() {
    let view = notice(
        ConversationNoticeKind::QueuedUser,
        "检查布局\n{\"literal\":true}",
        &[],
    );
    let full = strip_ansi_for_test(&render(&view, ToolCallDisplayMode::Full));
    assert!(full.contains("Queued message delivered"), "{full}");
    assert!(
        full.contains("    Message: 检查布局\n             {\"literal\":true}"),
        "{full}"
    );
    let summary = strip_ansi_for_test(&render(&view, ToolCallDisplayMode::Summary));
    assert!(
        summary.contains("Queued message delivered · 检查布局"),
        "{summary}"
    );
    assert!(!summary.contains('\n'));
}

/// 【终端】【通知图片测试】历史排队消息图片沿用 Kitty 协议，收起时不传输，展开时靠左并保留占位。
/// 参数: 无；返回: 无。
#[test]
fn queued_notice_images_are_foldable_left_aligned_and_reserved() {
    use crate::render::terminal_image::{reserved_image_rows, test_override};
    test_override::set(Some(true), Some(false), Some(false));
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("queued.png");
    image::RgbaImage::from_pixel(160, 160, image::Rgba([40, 120, 180, 255]))
        .save(&path)
        .unwrap();
    let view = notice(
        ConversationNoticeKind::QueuedUser,
        "Inspect attachment",
        &[path.to_string_lossy().into_owned()],
    );
    let collapsed = render_expand::with_force_collapse(|| render(&view, ToolCallDisplayMode::Full));
    assert!(!collapsed.contains("a=p"));
    let expanded = render_expand::with_force_expand(|| render(&view, ToolCallDisplayMode::Full));
    let (row, height, line) = expanded
        .split('\n')
        .enumerate()
        .find_map(|(row, line)| reserved_image_rows(line).map(|height| (row, height, line)))
        .unwrap();
    assert!(expanded.split('\n').count() >= row + height);
    assert!(line.starts_with("    \x1b_G"), "{line:?}");
    assert!(!expanded.contains("image_sources"));
    assert!(!expanded.contains("queued.png"));
    test_override::set(None, None, None);
}
