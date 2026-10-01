//! 输入队列回归测试：覆盖事件积压、滚轮累计和关键输入边界。

use super::input_queue::InputQueue;
use super::protocol::ClientMessage;
use crate::browser::MouseInput;

/// 【浏览器面板测试】【鼠标构造】按指定种类和坐标构造测试输入。
/// @param kind 为事件种类；x 为横坐标
/// @returns 鼠标消息
fn mouse(kind: &str, x: f64) -> ClientMessage {
    ClientMessage::Mouse(MouseInput {
        kind: kind.into(),
        x,
        y: 10.0,
        button: None,
        click_count: None,
        delta_x: 0.0,
        delta_y: 5.0,
        modifiers: 0,
    })
}

/// 【浏览器面板测试】【积压合并】执行中的命令不阻止合并后续移动，点击仍按顺序执行。
#[tokio::test]
async fn blocked_consumer_keeps_latest_move_before_click() {
    let queue = InputQueue::default();
    queue.push(mouse("move", 0.0)).unwrap();
    // 1. 模拟执行器已经取走一条移动并等待 CDP 响应
    let _in_flight = queue.next().await;
    for x in 1..=120 {
        queue.push(mouse("move", x as f64)).unwrap();
    }
    queue.push(mouse("down", 120.0)).unwrap();
    queue.push(mouse("up", 120.0)).unwrap();
    queue.push(mouse("move", 121.0)).unwrap();
    // 2. 积压只剩最新移动和关键边界，不需要执行旧的 119 次移动
    for (kind, x) in [
        ("move", 120.0),
        ("down", 120.0),
        ("up", 120.0),
        ("move", 121.0),
    ] {
        let ClientMessage::Mouse(input) = queue.next().await else {
            panic!("expected mouse")
        };
        assert_eq!(input.kind, kind);
        assert_eq!(input.x, x);
    }
}

/// 【浏览器面板测试】【滚轮累计】同一位置累计滚动距离，跨位置或键盘边界时不合并。
#[tokio::test]
async fn wheel_preserves_distance_and_target_boundaries() {
    let queue = InputQueue::default();
    for _ in 0..100 {
        queue.push(mouse("wheel", 10.0)).unwrap();
    }
    queue.push(mouse("wheel", 20.0)).unwrap();
    queue
        .push(ClientMessage::InsertText {
            text: "输入".into(),
        })
        .unwrap();
    queue.push(mouse("wheel", 20.0)).unwrap();
    let ClientMessage::Mouse(first) = queue.next().await else {
        panic!("expected mouse")
    };
    assert_eq!(first.delta_y, 500.0);
    let ClientMessage::Mouse(second) = queue.next().await else {
        panic!("expected mouse")
    };
    assert_eq!(second.x, 20.0);
    assert_eq!(second.delta_y, 5.0);
    assert!(matches!(
        queue.next().await,
        ClientMessage::InsertText { .. }
    ));
    assert!(matches!(queue.next().await, ClientMessage::Mouse(_)));
}

/// 【浏览器面板测试】【拖拽边界】按键状态发生变化的移动不能覆盖彼此。
#[tokio::test]
async fn dragging_and_modifier_changes_are_not_merged() {
    let queue = InputQueue::default();
    queue.push(mouse("move", 1.0)).unwrap();
    let ClientMessage::Mouse(mut drag) = mouse("move", 2.0) else {
        unreachable!()
    };
    drag.button = Some("left".into());
    queue.push(ClientMessage::Mouse(drag.clone())).unwrap();
    drag.modifiers = 8;
    queue.push(ClientMessage::Mouse(drag)).unwrap();
    for expected in [1.0, 2.0, 2.0] {
        let ClientMessage::Mouse(input) = queue.next().await else {
            panic!("expected mouse")
        };
        assert_eq!(input.x, expected);
    }
}

/// 【浏览器面板测试】【队列上限】无法合并的关键输入达到上限时明确拒绝，不静默丢失。
#[test]
fn queue_rejects_overflow_without_unbounded_growth() {
    let queue = InputQueue::default();
    for _ in 0..256 {
        queue.push(mouse("down", 0.0)).unwrap();
    }
    assert!(queue.push(mouse("up", 0.0)).is_err());
}
