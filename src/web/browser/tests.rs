use super::protocol::{ClientMessage, ServerMessage};
use crate::browser::BrowserState;

/// 【浏览器面板测试】【协议解析】面板的导航、鼠标、键盘与视口消息按 snake_case 解析。
#[test]
fn client_messages_parse_from_panel_json() {
    let navigate: ClientMessage =
        serde_json::from_str(r#"{"type":"navigate","url":"example.com"}"#).unwrap();
    assert!(matches!(navigate, ClientMessage::Navigate { ref url } if url == "example.com"));
    assert!(navigate.is_slow());
    let mouse: ClientMessage = serde_json::from_str(
        r#"{"type":"mouse","kind":"down","x":10.5,"y":20,"button":"left","click_count":2,"modifiers":8}"#,
    )
    .unwrap();
    let ClientMessage::Mouse(input) = &mouse else {
        panic!("expected mouse message");
    };
    assert_eq!(input.click_count, Some(2));
    assert_eq!(input.modifiers, 8);
    assert!(!mouse.is_slow());
    let key: ClientMessage = serde_json::from_str(
        r#"{"type":"key","kind":"down","key":"a","code":"KeyA","key_code":65,"text":"a"}"#,
    )
    .unwrap();
    assert!(matches!(key, ClientMessage::Key(ref input) if input.key_code == 65));
    let resize: ClientMessage =
        serde_json::from_str(r#"{"type":"resize","width":900,"height":600}"#).unwrap();
    assert!(matches!(
        resize,
        ClientMessage::Resize {
            width: 900,
            height: 600,
            scale,
        } if scale == 1.0
    ));
    assert!(serde_json::from_str::<ClientMessage>(r#"{"type":"unknown"}"#).is_err());
}

/// 【浏览器面板测试】【像素比协议】解析高分屏像素比，连续调整保留最新尺寸和像素比。
/// @returns 无
#[tokio::test]
async fn resize_queue_keeps_the_latest_device_scale() {
    let queue = super::input_queue::InputQueue::default();
    for message in [
        r#"{"type":"resize","width":900,"height":600,"scale":2}"#,
        r#"{"type":"resize","width":800,"height":500,"scale":1.5}"#,
    ] {
        queue.push(serde_json::from_str(message).unwrap()).unwrap();
    }
    assert!(matches!(
        queue.next().await,
        ClientMessage::Resize { width: 800, height: 500, scale } if scale == 1.5
    ));
}

/// 【浏览器面板测试】【状态序列化】状态消息携带类型标记与面板所需字段。
#[test]
fn server_state_message_serializes_with_type_tag() {
    let state = BrowserState {
        url: "https://example.com/".into(),
        title: "Example".into(),
        can_go_back: true,
        width: 1280,
        height: 800,
        ..BrowserState::default()
    };
    let value = serde_json::to_value(ServerMessage::State { state: &state }).unwrap();
    assert_eq!(value["type"], "state");
    assert_eq!(value["state"]["url"], "https://example.com/");
    assert_eq!(value["state"]["can_go_back"], true);
    assert_eq!(value["state"]["tabs"], serde_json::json!([]));
    let error = serde_json::to_value(ServerMessage::Error { message: "x" }).unwrap();
    assert_eq!(error, serde_json::json!({"type":"error","message":"x"}));
}

/// 【浏览器面板测试】【交互协议】对话框、下拉框、文件选择与元素选择的回复按 snake_case 解析。
#[test]
fn page_interaction_replies_parse_from_panel_json() {
    let reply: ClientMessage =
        serde_json::from_str(r#"{"type":"dialog_reply","accept":true,"prompt_text":"hi"}"#)
            .unwrap();
    assert!(
        matches!(reply, ClientMessage::DialogReply { accept: true, prompt_text: Some(ref text) } if text == "hi")
    );
    let select: ClientMessage =
        serde_json::from_str(r#"{"type":"select_reply","index":2}"#).unwrap();
    assert!(matches!(select, ClientMessage::SelectReply { index: 2 }));
    let files: ClientMessage =
        serde_json::from_str(r#"{"type":"file_chooser_reply","uploads":[]}"#).unwrap();
    assert!(files.is_slow(), "上传回复要等待页面处理，不能阻塞输入");
    let pick: ClientMessage =
        serde_json::from_str(r#"{"type":"pick_start","labels":["背景","颜色","字体"]}"#).unwrap();
    assert!(pick.is_slow(), "元素选择持续等待用户点击，必须放到独立任务");
    let cancel: ClientMessage = serde_json::from_str(r#"{"type":"pick_cancel"}"#).unwrap();
    assert!(!cancel.is_slow());
    // 对话框回复与取消选择不能排在被对话框卡住的点击后面
    assert!(reply.bypasses_queue() && cancel.bypasses_queue());
    assert!(!select.bypasses_queue() && !pick.bypasses_queue());
}

/// 【浏览器面板测试】【事件编码】关闭类消息序列化为 null，面板据此收起界面。
#[test]
fn closing_events_serialize_as_null_payloads() {
    use crate::browser::BrowserEvent;
    let encoded = super::event_messages::encode(&BrowserEvent::Dialog(None)).unwrap();
    let axum::extract::ws::Message::Text(text) = encoded else {
        panic!("expected text frame");
    };
    assert_eq!(text, r#"{"type":"dialog","dialog":null}"#);
    assert!(super::event_messages::encode(&BrowserEvent::Activity("x".into())).is_none());
}
