//! Chrome DevTools Protocol 的 WebSocket 客户端，按扁平化 sessionId 复用单条连接。

use anyhow::{anyhow, bail, Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, oneshot};
use tokio_tungstenite::tungstenite::protocol::{Message, WebSocketConfig};

/// 单条命令等待响应的默认上限。
pub(super) const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
/// 截图与录屏帧可能达到数 MB，放宽单条消息上限。
const MAX_MESSAGE_BYTES: usize = 256 << 20;

/// 浏览器推送的一条协议事件。
#[derive(Clone, Debug)]
pub(super) struct CdpEvent {
    /// 事件名，如 `Page.loadEventFired`
    pub(super) method: String,
    /// 事件参数
    pub(super) params: Value,
    /// 所属目标会话；浏览器级事件为空
    pub(super) session_id: Option<String>,
}

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value>>>>>;

/// 共享的 CDP 连接，可在多个任务间克隆使用。
#[derive(Clone)]
pub(super) struct CdpClient {
    outgoing: mpsc::UnboundedSender<String>,
    pending: Pending,
    next_id: Arc<AtomicU64>,
    events: broadcast::Sender<CdpEvent>,
    alive: Arc<AtomicBool>,
}

impl CdpClient {
    /// 【CDP】【建立连接】连接浏览器级调试地址并启动读写任务。
    /// @param url 为 `ws://127.0.0.1:<port>/devtools/browser/<id>`
    /// @returns 已连接的客户端
    pub(super) async fn connect(url: &str) -> Result<Self> {
        // 1. 放宽消息与帧大小限制后建立 WebSocket
        let mut config = WebSocketConfig::default();
        config.max_message_size = Some(MAX_MESSAGE_BYTES);
        config.max_frame_size = Some(MAX_MESSAGE_BYTES);
        let (stream, _) = tokio_tungstenite::connect_async_with_config(url, Some(config), false)
            .await
            .with_context(|| format!("connect DevTools {url}"))?;
        let (mut sink, mut source) = stream.split();
        let (outgoing, mut outgoing_rx) = mpsc::unbounded_channel::<String>();
        let pending: Pending = Arc::default();
        let (events, _) = broadcast::channel(512);
        let alive = Arc::new(AtomicBool::new(true));
        // 2. 写任务：把排队的命令依次写入连接
        tokio::spawn(async move {
            while let Some(text) = outgoing_rx.recv().await {
                if sink.send(Message::Text(text)).await.is_err() {
                    break;
                }
            }
            let _ = sink.close().await;
        });
        // 3. 读任务：响应按 id 交还调用方，其余消息作为事件广播
        let reader_pending = pending.clone();
        let reader_events = events.clone();
        let reader_alive = alive.clone();
        tokio::spawn(async move {
            while let Some(Ok(message)) = source.next().await {
                let Message::Text(text) = message else {
                    continue;
                };
                let Ok(value) = serde_json::from_str::<Value>(&text) else {
                    continue;
                };
                dispatch_message(value, &reader_pending, &reader_events);
            }
            // 4. 连接断开后让所有等待中的命令立即失败
            reader_alive.store(false, Ordering::SeqCst);
            let waiters: Vec<_> = reader_pending.lock().unwrap().drain().collect();
            for (_, waiter) in waiters {
                let _ = waiter.send(Err(anyhow!("DevTools connection closed")));
            }
        });
        Ok(Self {
            outgoing,
            pending,
            next_id: Arc::new(AtomicU64::new(1)),
            events,
            alive,
        })
    }

    /// 【CDP】【连接状态】判断连接是否仍然可用。
    /// @returns 读任务未退出时为 true
    pub(super) fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    /// 【CDP】【事件订阅】订阅浏览器推送的全部协议事件。
    /// @returns 事件接收端
    pub(super) fn subscribe(&self) -> broadcast::Receiver<CdpEvent> {
        self.events.subscribe()
    }

    /// 【CDP】【命令发送】发送命令并按默认超时等待结果。
    /// @param session_id 为目标会话，浏览器级命令传空；method 为命令名；params 为参数
    /// @returns 命令返回的 result 对象
    pub(super) async fn send(
        &self,
        session_id: Option<&str>,
        method: &str,
        params: Value,
    ) -> Result<Value> {
        self.send_with_timeout(session_id, method, params, COMMAND_TIMEOUT)
            .await
    }

    /// 【CDP】【限时命令】发送命令并在指定时长内等待结果。
    /// @param session_id 为目标会话；method 为命令名；params 为参数；timeout 为等待上限
    /// @returns 命令返回的 result 对象
    pub(super) async fn send_with_timeout(
        &self,
        session_id: Option<&str>,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value> {
        if !self.is_alive() {
            bail!("DevTools connection closed");
        }
        // 1. 分配请求 id 并登记等待者
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (sender, receiver) = oneshot::channel();
        self.pending.lock().unwrap().insert(id, sender);
        let mut message = json!({ "id": id, "method": method, "params": params });
        if let Some(session_id) = session_id {
            message["sessionId"] = json!(session_id);
        }
        // 2. 写入发送队列；失败时撤销登记
        if self.outgoing.send(message.to_string()).is_err() {
            self.pending.lock().unwrap().remove(&id);
            bail!("DevTools connection closed");
        }
        // 3. 等待响应，超时后撤销登记避免泄漏
        match tokio::time::timeout(timeout, receiver).await {
            Ok(Ok(result)) => result.with_context(|| format!("CDP {method}")),
            Ok(Err(_)) => bail!("DevTools connection closed during {method}"),
            Err(_) => {
                self.pending.lock().unwrap().remove(&id);
                bail!("CDP {method} timed out after {timeout:?}")
            }
        }
    }

    /// 【CDP】【单向命令】发送不关心结果的命令，如录屏帧确认。
    /// @param session_id 为目标会话；method 为命令名；params 为参数
    /// @returns 无
    pub(super) fn fire(&self, session_id: Option<&str>, method: &str, params: Value) {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let mut message = json!({ "id": id, "method": method, "params": params });
        if let Some(session_id) = session_id {
            message["sessionId"] = json!(session_id);
        }
        let _ = self.outgoing.send(message.to_string());
    }
}

/// 【CDP】【消息分发】把一条入站消息交给等待者或事件订阅方。
/// @param value 为解析后的消息；pending 为等待表；events 为事件广播
/// @returns 无
fn dispatch_message(value: Value, pending: &Pending, events: &broadcast::Sender<CdpEvent>) {
    if let Some(id) = value.get("id").and_then(Value::as_u64) {
        let Some(waiter) = pending.lock().unwrap().remove(&id) else {
            return;
        };
        let result = match value.get("error") {
            Some(error) => Err(anyhow!(
                "{}",
                error
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown DevTools error")
            )),
            None => Ok(value.get("result").cloned().unwrap_or(Value::Null)),
        };
        let _ = waiter.send(result);
        return;
    }
    let Some(method) = value.get("method").and_then(Value::as_str) else {
        return;
    };
    let _ = events.send(CdpEvent {
        method: method.to_string(),
        params: value.get("params").cloned().unwrap_or(Value::Null),
        session_id: value
            .get("sessionId")
            .and_then(Value::as_str)
            .map(str::to_string),
    });
}
