//! 把浏览器会话事件与连接初始状态编码为面板消息。

use super::frame_delivery::text;
use super::protocol::ServerMessage;
use crate::browser::{BrowserEvent, BrowserSession, FileChooserInfo};
use axum::extract::ws::Message;

/// 【浏览器面板】【事件编码】把对话框、下拉框、文件选择、下载与剪贴板事件转换为文本帧。
/// @param event 为会话事件；状态、画面与活动由调用方单独处理
/// @returns 文本帧；不属于这几类时为空
pub(super) fn encode(event: &BrowserEvent) -> Option<Message> {
    let message = match event {
        BrowserEvent::Dialog(dialog) => ServerMessage::Dialog {
            dialog: dialog.as_ref(),
        },
        BrowserEvent::SelectPopup(popup) => ServerMessage::SelectPopup { popup },
        BrowserEvent::FileChooser(chooser) => ServerMessage::FileChooser {
            chooser: chooser.as_ref(),
        },
        BrowserEvent::Download(download) => ServerMessage::Download { download },
        BrowserEvent::Clipboard(copied) => ServerMessage::Clipboard { text: copied },
        BrowserEvent::State(_) | BrowserEvent::Frame(_) | BrowserEvent::Activity(_) => return None,
    };
    Some(text(&message))
}

/// 【浏览器面板】【连接初始化】生成新连接需要补发的消息。
///
/// 只有本机访问时调试工具地址才有意义，由前端按访问地址决定是否显示入口。
///
/// @param session 为浏览器会话
/// @returns 连接信息、等待中的对话框与文件选择
pub(super) fn initial(session: &BrowserSession) -> Vec<Message> {
    let mut messages = vec![text(&ServerMessage::Info {
        devtools_url: session.devtools_url(),
        persistent_profile: session.uses_persistent_profile(),
    })];
    if let Some(dialog) = session.pending_dialog() {
        messages.push(text(&ServerMessage::Dialog {
            dialog: Some(&dialog),
        }));
    }
    if session.file_chooser_pending() {
        let chooser = FileChooserInfo::default();
        messages.push(text(&ServerMessage::FileChooser {
            chooser: Some(&chooser),
        }));
    }
    messages
}
