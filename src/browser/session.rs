//! 浏览器会话：持有浏览器进程、CDP 连接、当前标签页与面板订阅。

use super::cdp::CdpClient;
use super::events::{BrowserEvent, BrowserState};
use super::launcher::{LaunchedBrowser, MAX_DEVICE_SCALE};
use anyhow::{anyhow, bail, Result};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::AtomicUsize;
use std::sync::Mutex;
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
    /// 设备像素比：面板所在屏幕的 devicePixelRatio，高分屏上按此倍数渲染才清晰
    pub(super) scale: f64,
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
    /// 当前打开的页面对话框及其所在 sessionId
    pub(super) dialog: Option<(String, super::events::DialogInfo)>,
    /// 等待面板选择文件的 input 节点：(sessionId, backendNodeId)
    pub(super) file_chooser: Option<(String, i64)>,
    /// 最近的下载记录
    pub(super) downloads: Vec<super::events::DownloadInfo>,
}

/// 进程内共享的浏览器会话。
pub(crate) struct BrowserSession {
    pub(super) client: CdpClient,
    pub(super) inner: Mutex<SessionInner>,
    pub(super) events: broadcast::Sender<BrowserEvent>,
    /// 当前连接的面板数量，大于 0 时才开启录屏
    pub(super) viewers: AtomicUsize,
    pub(super) process: tokio::sync::Mutex<Option<LaunchedBrowser>>,
    /// 调试端口，用于打开 DevTools 前端
    pub(super) debug_port: u16,
    /// 是否使用持久用户目录
    pub(super) persistent_profile: bool,
}

impl BrowserSession {
    /// 【内置浏览器】【存活判断】判断 CDP 连接是否仍然可用。
    /// @returns 可用时为 true
    pub(crate) fn is_alive(&self) -> bool {
        self.client.is_alive()
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

    /// 【内置浏览器】【视口调整】按面板尺寸与设备像素比调整页面视口并重启录屏。
    /// @param width 为宽度；height 为高度，单位 CSS 像素；scale 为设备像素比
    /// @returns 操作结果
    pub(crate) async fn resize(&self, width: u32, height: u32, scale: f64) -> Result<()> {
        let width = width.clamp(320, 3840);
        let height = height.clamp(240, 2160);
        let scale = clamp_scale(scale);
        {
            let mut inner = self.inner.lock().unwrap();
            if inner.width == width && inner.height == height && inner.scale == scale {
                return Ok(());
            }
            inner.width = width;
            inner.height = height;
            inner.scale = scale;
        }
        let session = self.active_session()?;
        self.apply_viewport(&session).await?;
        self.restart_screencast().await;
        self.refresh_state().await;
        Ok(())
    }

    /// 【内置浏览器】【像素比】返回当前设备像素比。
    /// @returns 设备像素比，未设置时为 1
    pub(crate) fn device_scale(&self) -> f64 {
        clamp_scale(self.inner.lock().unwrap().scale)
    }

    /// 【内置浏览器】【视口应用】把记录的视口尺寸写入指定目标会话。
    /// @param session 为 sessionId
    /// @returns 操作结果
    pub(super) async fn apply_viewport(&self, session: &str) -> Result<()> {
        let (width, height) = self.viewport();
        let scale = self.device_scale();
        // 无头模式的 screen 默认是 800x600，比视口还小，站点验证会据此识别；
        // 屏幕尺寸取常见桌面分辨率与视口中的较大者
        self.client
            .send(
                Some(session),
                "Emulation.setDeviceMetricsOverride",
                json!({
                    "width": width,
                    "height": height,
                    "deviceScaleFactor": scale,
                    "mobile": false,
                    "screenWidth": width.max(1920),
                    "screenHeight": height.max(1080),
                }),
            )
            .await?;
        Ok(())
    }
}

/// 【内置浏览器】【像素比限制】把面板上报的像素比限制在 1 到 2 之间，非法值按 1 处理。
/// @param scale 为面板上报的 devicePixelRatio
/// @returns 可用的像素比
pub(super) fn clamp_scale(scale: f64) -> f64 {
    if scale.is_finite() && scale > 0.0 {
        // 【内置浏览器】【像素比限制】保留两位小数，避免浮点抖动导致反复重设视口
        ((scale.clamp(1.0, MAX_DEVICE_SCALE)) * 100.0).round() / 100.0
    } else {
        1.0
    }
}
