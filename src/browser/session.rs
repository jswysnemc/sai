//! 浏览器会话：持有浏览器进程、CDP 连接、当前标签页与面板订阅。

use super::cdp::CdpClient;
use super::events::{BrowserEvent, BrowserState};
use super::launcher::{self, LaunchedBrowser, DEFAULT_HEIGHT, DEFAULT_WIDTH};
use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

/// 会话内可变状态。
#[derive(Default)]
pub(super) struct SessionInner {
    /// 当前操作的目标 ID
    pub(super) active_target: String,
    /// 目标 ID 到扁平化 sessionId 的映射
    pub(super) attached: HashMap<String, String>,
    /// 视口宽度（CSS 像素）
    pub(super) width: u32,
    /// 视口高度（CSS 像素）
    pub(super) height: u32,
    /// 当前主框架是否在加载
    pub(super) loading: bool,
    /// 最近一次广播的状态，用于去重与新面板首屏
    pub(super) last_state: BrowserState,
    /// 正在录屏的 sessionId
    pub(super) screencast_session: Option<String>,
    /// 已排队一次状态汇总，合并窗口内的后续事件不再排队
    pub(super) refresh_pending: bool,
    /// sessionId 到 Sai 隔离世界执行上下文 ID 的映射，导航后失效
    pub(super) isolated_contexts: HashMap<String, i64>,
}

/// 进程内共享的浏览器会话。
pub(crate) struct BrowserSession {
    pub(super) client: CdpClient,
    pub(super) inner: Mutex<SessionInner>,
    pub(super) events: broadcast::Sender<BrowserEvent>,
    /// 当前连接的面板数量，大于 0 时才开启录屏
    pub(super) viewers: AtomicUsize,
    process: tokio::sync::Mutex<Option<LaunchedBrowser>>,
}

impl BrowserSession {
    /// 【内置浏览器】【会话启动】启动浏览器、建立连接并附着到首个页面。
    /// @returns 共享会话
    pub(super) async fn launch() -> Result<Arc<Self>> {
        // 1. 启动进程并连接浏览器级调试地址
        let launched = launcher::launch().await?;
        let client = CdpClient::connect(&launched.websocket_url).await?;
        let (events, _) = broadcast::channel(64);
        let session = Arc::new(Self {
            client,
            inner: Mutex::new(SessionInner {
                width: DEFAULT_WIDTH,
                height: DEFAULT_HEIGHT,
                ..SessionInner::default()
            }),
            events,
            viewers: AtomicUsize::new(0),
            process: tokio::sync::Mutex::new(Some(launched)),
        });
        // 2. 打开目标发现，标签页增删与标题变化都会推送事件
        session
            .client
            .send(
                None,
                "Target.setDiscoverTargets",
                json!({ "discover": true }),
            )
            .await?;
        // 3. 事件泵先于附着启动，避免漏掉首个页面的加载事件
        session.spawn_event_pump();
        let target = match session.page_targets().await?.into_iter().next() {
            Some(target) => target.target_id,
            None => session.create_target("about:blank").await?,
        };
        session.activate_target(&target).await?;
        Ok(session)
    }

    /// 【内置浏览器】【存活判断】判断 CDP 连接是否仍然可用。
    /// @returns 可用时为 true
    pub(crate) fn is_alive(&self) -> bool {
        self.client.is_alive()
    }

    /// 【内置浏览器】【进程关闭】请求浏览器正常退出，随后强制结束进程。
    /// @returns 无
    pub(crate) async fn close(&self) {
        let _ = self
            .client
            .send_with_timeout(
                None,
                "Browser.close",
                json!({}),
                std::time::Duration::from_secs(2),
            )
            .await;
        let Some(mut launched) = self.process.lock().await.take() else {
            return;
        };
        // 1. 给浏览器留出正常退出的时间，超时再强制结束
        let exited = tokio::time::timeout(std::time::Duration::from_secs(3), launched.child.wait())
            .await
            .is_ok();
        if !exited {
            let _ = launched.child.kill().await;
        }
        // 2. 渲染等子进程可能仍在写用户目录，删除失败时短暂重试
        let LaunchedBrowser {
            _profile: profile, ..
        } = launched;
        let path = profile.keep();
        for _ in 0..10 {
            if tokio::fs::remove_dir_all(&path).await.is_ok() || !path.exists() {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    }

    /// 【内置浏览器】【事件订阅】订阅面板所需的状态与画面事件。
    /// @returns 事件接收端
    pub(crate) fn subscribe(&self) -> broadcast::Receiver<BrowserEvent> {
        self.events.subscribe()
    }

    /// 【内置浏览器】【活动广播】通知面板 Agent 正在执行的操作。
    /// @param message 为操作摘要
    /// @returns 无
    pub(crate) fn emit_activity(&self, message: impl Into<String>) {
        let _ = self.events.send(BrowserEvent::Activity(message.into()));
    }

    /// 【内置浏览器】【当前会话】返回当前标签页的 sessionId。
    /// @returns sessionId；尚未附着时报错
    pub(super) fn active_session(&self) -> Result<String> {
        let inner = self.inner.lock().unwrap();
        inner
            .attached
            .get(&inner.active_target)
            .cloned()
            .ok_or_else(|| anyhow!("no active browser tab"))
    }

    /// 【内置浏览器】【页面命令】向当前标签页发送一条 CDP 命令。
    /// @param method 为命令名；params 为参数
    /// @returns 命令结果
    pub(super) async fn page_send(&self, method: &str, params: Value) -> Result<Value> {
        let session = self.active_session()?;
        self.client.send(Some(&session), method, params).await
    }

    /// 【内置浏览器】【脚本求值】在当前页面主世界执行表达式并按值返回结果。
    ///
    /// 主世界与页面脚本共享全局对象，只用于读取页面自身状态与 browser action=evaluate；
    /// 快照与 ref 定位走隔离世界，见 `evaluate_isolated`。
    ///
    /// @param expression 为 JavaScript 表达式，可返回 Promise
    /// @returns 表达式结果；页面抛出异常时报错
    pub(crate) async fn evaluate(&self, expression: &str) -> Result<Value> {
        let session = self.active_session()?;
        self.evaluate_in(&session, None, expression).await
    }

    /// 【内置浏览器】【上下文求值】在指定执行上下文中求值，上下文为空时使用主世界。
    /// @param session 为 sessionId；context 为执行上下文 ID；expression 为表达式
    /// @returns 表达式结果；页面抛出异常时报错
    pub(super) async fn evaluate_in(
        &self,
        session: &str,
        context: Option<i64>,
        expression: &str,
    ) -> Result<Value> {
        let mut params = json!({
            "expression": expression,
            "returnByValue": true,
            "awaitPromise": true,
            "userGesture": true,
        });
        if let Some(context) = context {
            params["contextId"] = json!(context);
        }
        let result = self
            .client
            .send(Some(session), "Runtime.evaluate", params)
            .await?;
        if let Some(details) = result.get("exceptionDetails") {
            let message = details
                .pointer("/exception/description")
                .or_else(|| details.get("text"))
                .and_then(Value::as_str)
                .unwrap_or("script error");
            bail!("{}", message.lines().next().unwrap_or(message));
        }
        Ok(result
            .pointer("/result/value")
            .cloned()
            .unwrap_or(Value::Null))
    }

    /// 【内置浏览器】【视口尺寸】返回当前视口宽高。
    /// @returns (宽, 高)，单位 CSS 像素
    pub(crate) fn viewport(&self) -> (u32, u32) {
        let inner = self.inner.lock().unwrap();
        (inner.width, inner.height)
    }

    /// 【内置浏览器】【视口调整】按面板尺寸调整页面视口并重启录屏。
    /// @param width 为宽度；height 为高度，单位 CSS 像素
    /// @returns 操作结果
    pub(crate) async fn resize(&self, width: u32, height: u32) -> Result<()> {
        let width = width.clamp(320, 3840);
        let height = height.clamp(240, 2160);
        {
            let mut inner = self.inner.lock().unwrap();
            if inner.width == width && inner.height == height {
                return Ok(());
            }
            inner.width = width;
            inner.height = height;
        }
        let session = self.active_session()?;
        self.apply_viewport(&session).await?;
        self.restart_screencast().await;
        self.refresh_state().await;
        Ok(())
    }

    /// 【内置浏览器】【视口应用】把记录的视口尺寸写入指定目标会话。
    /// @param session 为 sessionId
    /// @returns 操作结果
    pub(super) async fn apply_viewport(&self, session: &str) -> Result<()> {
        let (width, height) = self.viewport();
        self.client
            .send(
                Some(session),
                "Emulation.setDeviceMetricsOverride",
                json!({
                    "width": width,
                    "height": height,
                    "deviceScaleFactor": 1,
                    "mobile": false,
                }),
            )
            .await?;
        Ok(())
    }
}
