use super::test_support::*;
use serde_json::json;

/// 【桌面测试】【控件读取】读取值与查找文本不额外访问桌面，不消费同一观察的输入资格。
/// 参数: 无；返回: 无。
#[tokio::test]
async fn queries_use_the_current_snapshot_without_clicking_or_consuming_it() {
    let (mut session, driver) = session(vec![
        state(false),
        json!({"structuredContent":{"effect":"invoked"}}),
    ]);
    let observed = session.execute(observe_args()).await.unwrap();
    let mut value = input_args(&observed.content, "get_value");
    value["element_index"] = json!("g1:1");
    assert!(session
        .execute(value)
        .await
        .unwrap()
        .content
        .contains("\"value\":\"\""));
    let mut text = input_args(&observed.content, "get_text");
    text["element_index"] = json!("g1:0");
    assert!(session
        .execute(text)
        .await
        .unwrap()
        .content
        .contains("Apply"));
    let mut find = input_args(&observed.content, "locate_text");
    find["name"] = json!("apply");
    let found = session.execute(find.clone()).await.unwrap();
    assert!(found.content.contains("g1:0"));
    assert!(!found.content.contains("native-button-token"));
    assert_eq!(driver.calls.lock().unwrap().len(), 1);
    let mut invoke = input_args(&observed.content, "invoke");
    invoke["element_index"] = json!("g1:0");
    session.execute(invoke).await.unwrap();
    assert!(session.execute(find).await.is_err());
    let calls = driver.calls.lock().unwrap();
    assert_eq!(calls[1].0, "click");
    assert_eq!(calls[1].1["action"], "press");
    assert_eq!(calls[1].1["element_token"], "native-button-token");
}

/// 【桌面测试】【菜单动作】原生菜单调用必须使用当前元素证明，并且一次调用即使观察失效。
/// 参数: 无；返回: 无。
#[tokio::test]
async fn show_menu_uses_an_explicit_native_control_and_consumes_the_snapshot() {
    let (mut session, driver) = session(vec![
        state(false),
        json!({"structuredContent":{"effect":"opened"}}),
    ]);
    let observed = session.execute(observe_args()).await.unwrap();
    let mut menu = input_args(&observed.content, "show_menu");
    menu["element_index"] = json!("g1:0");
    session.execute(menu.clone()).await.unwrap();
    assert!(session.execute(menu).await.is_err());
    let calls = driver.calls.lock().unwrap();
    assert_eq!(calls[1].1["action"], "show_menu");
    assert_eq!(calls[1].1["delivery_mode"], "background");
}

/// 【桌面测试】【查询边界】跨窗口、旧句柄与缺少值的请求均明确失败，不猜测控件。
/// 参数: 无；返回: 无。
#[tokio::test]
async fn invalid_queries_never_guess_a_target() {
    let (mut session, driver) = session(vec![state(false)]);
    let observed = session.execute(observe_args()).await.unwrap();
    for (action, extra) in [
        ("get_value", json!({"element_index":"g1:0"})),
        ("get_text", json!({"element_index":"g99:0"})),
        ("get_text", json!({"element_index":"g1:0","window_id":102})),
        ("locate_text", json!({"name":" "})),
    ] {
        let mut args = input_args(&observed.content, action);
        args.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        assert!(session.execute(args).await.is_err());
    }
    assert_eq!(driver.calls.lock().unwrap().len(), 1);
}
