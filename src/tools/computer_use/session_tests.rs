use super::session::{ComputerSession, DesktopCoordinator};
use super::test_support::*;
use serde_json::json;
use std::sync::Arc;

/// 【桌面测试】【完整调用】观察保留原图，元素输入映射到原生 token 并消费快照。
#[tokio::test]
async fn observation_and_element_action_keep_the_native_contract() {
    let (mut session, driver) = session(vec![
        state(true),
        json!({"structuredContent":{"effect":"applied"}}),
    ]);
    let observed = session.execute(observe_args()).await.unwrap();
    assert_eq!(observed.model_attachments.len(), 1);
    assert_eq!(
        observed.model_attachments[0].image_url,
        format!("data:image/png;base64,{}", screenshot_base64())
    );
    assert!(observed.content.contains("g1:0"));
    assert!(!observed.content.contains("native-button-token"));
    let mut click = input_args(&observed.content, "click");
    click["element_index"] = json!("g1:0");
    let output = session.execute(click.clone()).await.unwrap();
    assert!(output.content.contains("applied"));
    assert!(output.content.contains("get_app_state"));
    let calls = driver.calls.lock().unwrap().clone();
    assert_eq!(calls[0].0, "get_window_state");
    assert_eq!(calls[0].1["max_image_dimension"], 1568);
    assert_eq!(
        calls[1],
        (
            "click".into(),
            json!({"pid":42,"window_id":101,"delivery_mode":"background","element_token":"native-button-token"})
        )
    );
    assert!(session
        .execute(click)
        .await
        .unwrap_err()
        .to_string()
        .contains("No current"));
    assert_eq!(driver.calls.lock().unwrap().len(), 2);
}

/// 【桌面测试】【像素边界】拒绝越界点和错误目标，正确坐标原样传递并绑定截图。
#[tokio::test]
async fn pixel_input_is_window_bound_and_never_rescaled() {
    let (mut session, driver) = session(vec![
        state(true),
        json!({"content":[{"type":"text","text":"accepted"}]}),
    ]);
    let observed = session.execute(observe_args()).await.unwrap();
    let mut click = input_args(&observed.content, "click");
    click["x"] = json!(100);
    click["y"] = json!(59);
    assert!(session
        .execute(click.clone())
        .await
        .unwrap_err()
        .to_string()
        .contains("out_of_bounds"));
    click["x"] = json!(99.5);
    click["window_id"] = json!(102);
    assert!(session
        .execute(click.clone())
        .await
        .unwrap_err()
        .to_string()
        .contains("target_mismatch"));
    click["window_id"] = json!(101);
    click["app"] = json!("43");
    assert!(session
        .execute(click.clone())
        .await
        .unwrap_err()
        .to_string()
        .contains("target_mismatch"));
    click["app"] = json!("42");
    session.execute(click).await.unwrap();
    let calls = driver.calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[1].1["x"], 99.5);
    assert_eq!(calls[1].1["y"], 59.0);
    assert_eq!(calls[1].1["coordinate_frame"], "window");
    assert_eq!(calls[1].1["capture_id"], "capture-fixture");
}

/// 【桌面测试】【失效句柄】新观察拒绝旧快照和旧元素，不把句柄当作裸数字使用。
#[tokio::test]
async fn new_state_invalidates_old_snapshot_and_handles() {
    let (mut session, driver) = session(vec![state(true), state(true)]);
    let old = session.execute(observe_args()).await.unwrap();
    let fresh = session.execute(observe_args()).await.unwrap();
    let mut stale = input_args(&old.content, "click");
    stale["element_index"] = json!("g1:0");
    assert!(session
        .execute(stale)
        .await
        .unwrap_err()
        .to_string()
        .contains("stale_snapshot"));
    let mut wrong_handle = input_args(&fresh.content, "click");
    wrong_handle["element_index"] = json!("g1:0");
    assert!(session
        .execute(wrong_handle)
        .await
        .unwrap_err()
        .to_string()
        .contains("stale_element"));
    assert_eq!(driver.calls.lock().unwrap().len(), 2);
}

/// 【桌面测试】【无图观察】截图缺失不阻断元素操作，但不能把屏幕坐标当作窗口坐标。
#[tokio::test]
async fn tree_only_state_allows_tokens_but_rejects_pixels() {
    let (mut session, driver) = session(vec![
        state(false),
        json!({"structuredContent":{"effect":"applied"}}),
    ]);
    let observed = session.execute(observe_args()).await.unwrap();
    assert!(observed.model_attachments.is_empty());
    assert!(observed.content.contains("pixel actions are disabled"));
    let mut click = input_args(&observed.content, "click");
    click["x"] = json!(1);
    click["y"] = json!(1);
    assert!(session
        .execute(click.clone())
        .await
        .unwrap_err()
        .to_string()
        .contains("screenshot_required"));
    click.as_object_mut().unwrap().remove("x");
    click.as_object_mut().unwrap().remove("y");
    click["element_index"] = json!("g1:0");
    session.execute(click).await.unwrap();
    assert_eq!(driver.calls.lock().unwrap().len(), 2);
}

/// 【桌面测试】【原生失败】拒绝与断连都保留失败语义，并禁止用同一快照盲目重试。
#[tokio::test]
async fn rejection_and_unknown_outcomes_are_not_replayed() {
    for failure in [
        json!({"isError":true,"content":[{"type":"text","text":"background_unavailable"}]}),
        json!({"transport_failure":true}),
        json!(null),
        json!({"unexpected":"not a tool result"}),
        json!({"isError":"false","content":[]}),
    ] {
        let (mut session, driver) = session(vec![state(true), failure]);
        let observed = session.execute(observe_args()).await.unwrap();
        let mut input = input_args(&observed.content, "type_text");
        input["text"] = json!("test");
        assert!(session.execute(input.clone()).await.is_err());
        assert!(session
            .execute(input)
            .await
            .unwrap_err()
            .to_string()
            .contains("No current"));
        assert_eq!(driver.calls.lock().unwrap().len(), 2);
    }
}

/// 【桌面测试】【跨会话失效】其他会话已经输入时，旧窗口快照不能继续发送动作。
#[tokio::test]
async fn another_sessions_input_invalidates_observations() {
    let coordinator = Arc::new(DesktopCoordinator::default());
    let first = Arc::new(FakeDriver::default());
    first.replies.lock().unwrap().push_back(state(true));
    let second = Arc::new(FakeDriver::default());
    second.replies.lock().unwrap().extend([
        state(true),
        json!({"structuredContent":{"effect":"applied"}}),
    ]);
    let mut a = ComputerSession::new(first.clone(), coordinator.clone());
    let mut b = ComputerSession::new(second, coordinator);
    let observed_a = a.execute(observe_args()).await.unwrap();
    let observed_b = b.execute(observe_args()).await.unwrap();
    let mut key = input_args(&observed_b.content, "press_key");
    key["key"] = json!("Tab");
    b.execute(key).await.unwrap();
    let mut stale = input_args(&observed_a.content, "press_key");
    stale["key"] = json!("Return");
    assert!(a
        .execute(stale)
        .await
        .unwrap_err()
        .to_string()
        .contains("stale_snapshot"));
    assert_eq!(first.calls.lock().unwrap().len(), 1);
}

/// 【桌面测试】【目标解析】应用重名时拒绝猜测，唯一名称可解析到确定进程。
#[tokio::test]
async fn app_discovery_requires_an_unambiguous_running_process() {
    let apps = json!({"structuredContent":{"apps":[
        {"name":"Editor","pid":42,"running":true}, {"name":"Editor","pid":43,"running":true},
        {"name":"Viewer","pid":44,"running":false}, {"name":"Terminal","pid":45,"running":true}
    ]}});
    let (mut session, driver) = session(vec![
        apps.clone(),
        apps.clone(),
        apps,
        json!({"structuredContent":{"windows":[]}}),
    ]);
    for name in ["Editor", "Viewer"] {
        assert!(session
            .execute(json!({"action":"list_windows","app":name}))
            .await
            .is_err());
    }
    session
        .execute(json!({"action":"list_windows","app":"Terminal"}))
        .await
        .unwrap();
    let calls = driver.calls.lock().unwrap();
    assert_eq!(
        calls.last().unwrap(),
        &("list_windows".into(), json!({"pid":45}))
    );
}

/// 【桌面测试】【状态校验】错误目标、图片损坏或尺寸不匹配都不能产生可操作快照。
#[tokio::test]
async fn malformed_or_misdirected_observations_are_rejected() {
    let mut wrong_target = state(true);
    wrong_target["structuredContent"]["window_id"] = json!(999);
    let mut wrong_size = state(true);
    wrong_size["structuredContent"]["screenshot_width"] = json!(200);
    let mut broken_image = state(true);
    broken_image["content"][1]["data"] = json!("not-an-image");
    let mut wrong_mime = state(true);
    wrong_mime["content"][1]["mimeType"] = json!("image/jpeg");
    for response in [wrong_target, wrong_size, broken_image, wrong_mime] {
        let (mut session, _) = session(vec![response]);
        assert!(session.execute(observe_args()).await.is_err());
        let click = json!({"action":"click","app":"42","window_id":101,"snapshot_id":"fake","element_index":"g1:0"});
        assert!(session
            .execute(click)
            .await
            .unwrap_err()
            .to_string()
            .contains("No current"));
    }
}

/// 【桌面测试】【重置会话】重置只关闭当前驱动，并清除可用的快照。
#[tokio::test]
async fn reset_closes_the_driver_and_clears_handles() {
    let (mut session, driver) = session(vec![state(true)]);
    let observed = session.execute(observe_args()).await.unwrap();
    session.execute(json!({"action":"reset"})).await.unwrap();
    assert_eq!(*driver.closed.lock().unwrap(), 1);
    let mut key = input_args(&observed.content, "press_key");
    key["key"] = json!("Return");
    assert!(session.execute(key).await.is_err());
}

/// 【桌面测试】【输入契约】每种输入只发送对应字段，整数形式的浮点值不会导致崩溃。
#[tokio::test]
async fn every_input_action_uses_the_expected_native_parameters() {
    for (action, extra, expected) in [
        (
            "click",
            json!({"x":2,"y":3,"click_count":2.0,"modifiers":["ctrl"]}),
            json!({"x":2.0,"y":3.0,"count":2,"modifier":["ctrl"],"coordinate_frame":"window","capture_id":"capture-fixture"}),
        ),
        (
            "scroll",
            json!({"direction":"down","amount":3.0,"by":"page"}),
            json!({"direction":"down","amount":3,"by":"page"}),
        ),
        (
            "drag",
            json!({"from_x":1,"from_y":2,"to_x":98,"to_y":59,"duration_ms":350.0}),
            json!({"from_x":1.0,"from_y":2.0,"to_x":98.0,"to_y":59.0,"duration_ms":350,"coordinate_frame":"window"}),
        ),
        (
            "press_key",
            json!({"key":"a","modifiers":["ctrl"]}),
            json!({"key":"a","modifiers":["ctrl"]}),
        ),
        (
            "type_text",
            json!({"element_index":"g1:1","text":"测试输入"}),
            json!({"element_token":"native-field-token","text":"测试输入"}),
        ),
        (
            "set_value",
            json!({"element_index":"g1:1","value":42.5}),
            json!({"element_token":"native-field-token","value":42.5}),
        ),
    ] {
        let (mut session, driver) = session(vec![
            state(true),
            json!({"structuredContent":{"effect":"applied"}}),
        ]);
        let mut observe = observe_args();
        observe["window_id"] = json!(101.0);
        observe["max_elements"] = json!(100.0);
        let observed = session.execute(observe).await.unwrap();
        let mut input = input_args(&observed.content, action);
        input["window_id"] = json!(101.0);
        input
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        session.execute(input).await.unwrap();
        let mut expected = expected;
        expected["pid"] = json!(42);
        expected["window_id"] = json!(101);
        expected["delivery_mode"] = json!("background");
        let calls = driver.calls.lock().unwrap();
        assert_eq!(calls[0].1["max_elements"], 100);
        assert_eq!(calls[1], (action.into(), expected));
    }
}

/// 【桌面测试】【发现断连】枚举期间断连后，也不能在重连时复用之前的观察发送按键。
#[tokio::test]
async fn inventory_failure_invalidates_previous_observation() {
    let (mut session, driver) = session(vec![state(true), json!({"transport_failure":true})]);
    let observed = session.execute(observe_args()).await.unwrap();
    assert!(session
        .execute(json!({"action":"list_apps"}))
        .await
        .is_err());
    let mut key = input_args(&observed.content, "press_key");
    key["key"] = json!("Return");
    assert!(session
        .execute(key)
        .await
        .unwrap_err()
        .to_string()
        .contains("No current"));
    assert_eq!(driver.calls.lock().unwrap().len(), 2);
}
