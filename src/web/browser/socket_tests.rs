//! 可选真实浏览器回归测试，覆盖 WebSocket 输入积压和实际视口尺寸。

use super::serve_socket;
use axum::{extract::ws::WebSocketUpgrade, routing::get, Router};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::time::{Duration, Instant};
use tokio_tungstenite::tungstenite::Message;

/// 【浏览器面板测试】【真实交互】启用 SAI_BROWSER_E2E 后验证高频移动不会拖延后续点击。
#[tokio::test]
async fn live_socket_coalesces_movement_and_resizes_when_enabled() {
    if std::env::var("SAI_BROWSER_E2E").ok().as_deref() != Some("1") {
        return;
    }
    let session = crate::browser::shared().await.unwrap();
    let html = r#"<style>body{margin:0}main{min-width:1250px;text-align:center}</style>
        <main><h1>Centered desktop page</h1></main><script>
        window.clicks=[];window.moves=0;
        addEventListener('mousemove',()=>window.moves++);
        addEventListener('mousedown',()=>window.clicks.push('down'));
        addEventListener('mouseup',()=>window.clicks.push('up'));
        </script>"#;
    session
        .navigate(&format!("data:text/html,{}", urlencoding::encode(html)))
        .await
        .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = Router::new().route(
        "/socket",
        get(|upgrade: WebSocketUpgrade| async { upgrade.on_upgrade(serve_socket) }),
    );
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let (socket, _) = tokio_tungstenite::connect_async(format!("ws://{address}/socket"))
        .await
        .unwrap();
    let (mut sender, mut receiver) = socket.split();
    // 1. 【浏览器面板测试】【尺寸同步】通过真实连接同步面板实际视口，并等到服务端确认
    sender
        .send(Message::Text(
            json!({"type":"resize","width":703,"height":1084}).to_string(),
        ))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(Ok(message)) = receiver.next().await {
            if let Message::Text(text) = message {
                let value: serde_json::Value = serde_json::from_str(&text).unwrap();
                if value["state"]["height"] == 1084 {
                    return;
                }
            }
        }
        panic!("socket closed before resize acknowledgement");
    })
    .await
    .unwrap();
    let metrics = session
        .evaluate("[innerWidth,innerHeight,document.documentElement.scrollWidth]")
        .await
        .unwrap();
    assert_eq!(metrics, json!([703, 1084, 1250]));
    let reader = tokio::spawn(async move { while receiver.next().await.is_some() {} });

    // 2. 【浏览器面板测试】【输入积压】以 120Hz 发送移动，随后立即点击，复现旧实现约半秒的队列尾部延迟
    let start = tokio::time::Instant::now();
    for index in 0..60 {
        tokio::time::sleep_until(start + Duration::from_micros(index * 8333)).await;
        let message = json!({"type":"mouse","kind":"move","x":10+index%5,"y":10});
        sender
            .send(Message::Text(message.to_string()))
            .await
            .unwrap();
    }
    let tail = Instant::now();
    for kind in ["down", "up"] {
        sender
            .send(Message::Text(
                json!({"type":"mouse","kind":kind,"x":10,"y":10,"button":"left"}).to_string(),
            ))
            .await
            .unwrap();
    }
    let completed = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if session.evaluate("window.clicks").await.unwrap() == json!(["down", "up"]) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    let elapsed = tail.elapsed();
    println!("120Hz input tail latency: {elapsed:?}");
    let _ = sender.close().await;
    reader.abort();
    server.abort();
    crate::browser::shutdown().await;
    assert!(completed.is_ok(), "click did not complete");
    assert!(
        elapsed < Duration::from_millis(200),
        "input backlog: {elapsed:?}"
    );
}
