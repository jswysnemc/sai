use super::*;
use crate::agent::AgentMode;
use crate::cli::repl_input::ReplInputSubmission;
use crate::render::input_atom::InputAtomKind;
use crate::state::input_history::{InputHistoryAttachment, InputHistoryAttachmentKind};

/// 【终端】【中断恢复】提交后恢复必须保留文本块样式、位置与再次发送的完整正文。
/// 参数: 无
/// 返回: 无；文本块变成普通标签或再次发送丢失内容时断言失败
#[test]
fn interrupted_submission_restores_large_paste_as_an_editable_atom() {
    let mut clipboard = ReplClipboardState::default();
    let mut input = "说明 ".to_string();
    let mut cursor = input.chars().count();
    clipboard.paste_text_into_input(&mut input, &mut cursor, "完整粘贴正文\n".repeat(4_000));
    input.push_str(" 结尾");
    let submission = ReplInputSubmission::from_input(AgentMode::Yolo, input.clone(), &clipboard);
    let mut text = None;
    let mut restored_clipboard = None;
    restore_submitted_input(&submission.history, &mut text, &mut restored_clipboard);
    let mut restored_clipboard = restored_clipboard.unwrap_or_default();
    let mut restored = text.unwrap();
    assert_eq!(restored, input);
    let spans = restored_clipboard.block_spans(&restored);
    assert_eq!(spans.len(), 1, "中断恢复后文本块必须保持特殊渲染");
    assert_eq!(spans[0].kind, InputAtomKind::Text);
    let resent = restored_clipboard.to_chat_input(&restored);
    assert_eq!(resent.message, submission.chat_input.message);
    cursor = spans[0].end;
    assert!(restored_clipboard.remove_block_before_cursor(&mut restored, &mut cursor));
    assert_eq!(restored, "说明  结尾");
}

/// 【终端】【失败恢复】恢复图片和文件、技能引用时使用当前快照，不混入上一份预填附件。
/// 参数: 无
/// 返回: 无；任何附件丢失或旧附件残留时断言失败
#[test]
fn restored_submission_keeps_images_and_references_together() {
    let entry = InputHistoryEntry {
        text: "[image 1 1x1] [@main.rs] [#review]".into(),
        attachments: vec![
            InputHistoryAttachment {
                marker: "[image 1 1x1]".into(),
                content: "data:image/png;base64,original".into(),
                kind: InputHistoryAttachmentKind::Image,
            },
            InputHistoryAttachment {
                marker: "[@main.rs]".into(),
                content: "@src/main.rs".into(),
                kind: InputHistoryAttachmentKind::File,
            },
            InputHistoryAttachment {
                marker: "[#review]".into(),
                content: "#review".into(),
                kind: InputHistoryAttachmentKind::Skill,
            },
        ],
    };
    let mut text = Some("stale".into());
    let mut clipboard = Some(ReplClipboardState::default());
    restore_submitted_input(&entry, &mut text, &mut clipboard);
    let clipboard = clipboard.unwrap_or_default();
    let restored = text.unwrap();
    assert_eq!(clipboard.block_spans(&restored).len(), 3);
    let resent = clipboard.to_chat_input(&restored);
    assert_eq!(
        resent.image_url.as_deref(),
        Some("data:image/png;base64,original")
    );
    assert!(resent.message.contains("@src/main.rs"));
    assert!(resent.message.contains("#review"));
}
