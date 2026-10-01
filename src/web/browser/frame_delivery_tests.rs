//! 使用可暂停写入端验证慢连接只发送最新画面，并优先发送控制消息。

use super::frame_delivery::deliver;
use axum::extract::ws::Message;
use futures_util::sink;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch, Semaphore};

/// 【浏览器面板测试】【慢连接】一帧写入受阻期间的新帧覆盖旧帧，恢复后控制消息优先。
#[tokio::test]
async fn slow_writer_skips_stale_frames_and_prioritizes_control() {
    let (observed_tx, mut observed_rx) = mpsc::unbounded_channel();
    let gate = Arc::new(Semaphore::new(0));
    let sink = Box::pin(sink::unfold(
        (observed_tx, gate.clone()),
        |(observed, gate), message| async move {
            observed.send(message).unwrap();
            gate.acquire().await.unwrap().forget();
            Ok::<_, Infallible>((observed, gate))
        },
    ));
    let (frames, frame_rx) = watch::channel(None);
    let (control, control_rx) = mpsc::unbounded_channel();
    let task = tokio::spawn(deliver(sink, frame_rx, control_rx));
    frames.send_replace(Some(Arc::new(vec![1])));
    let first = tokio::time::timeout(Duration::from_secs(1), observed_rx.recv())
        .await
        .unwrap();
    assert!(matches!(first, Some(Message::Binary(bytes)) if bytes == [1]));

    // 1. 第一帧发送尚未完成，模拟浏览器继续生成大量画面
    for value in 2..=100 {
        frames.send_replace(Some(Arc::new(vec![value])));
    }
    control.send(Message::Text("state".into())).unwrap();
    gate.add_permits(1);
    let state = tokio::time::timeout(Duration::from_secs(1), observed_rx.recv())
        .await
        .unwrap();
    assert!(matches!(state, Some(Message::Text(text)) if text == "state"));

    // 2. 控制消息之后直接发送最新一帧，不重放中间的 98 帧
    gate.add_permits(1);
    let latest = tokio::time::timeout(Duration::from_secs(1), observed_rx.recv())
        .await
        .unwrap();
    assert!(matches!(latest, Some(Message::Binary(bytes)) if bytes == [100]));
    task.abort();
}
