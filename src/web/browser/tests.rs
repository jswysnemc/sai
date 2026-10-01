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
            height: 600
        }
    ));
    assert!(serde_json::from_str::<ClientMessage>(r#"{"type":"unknown"}"#).is_err());
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
