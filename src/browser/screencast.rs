//! 事件泵与录屏：把 CDP 事件转换成面板状态，并在有面板连接时推送页面画面。

use super::cdp::CdpEvent;
use super::events::BrowserEvent;
use super::session::BrowserSession;
use base64::Engine;
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Weak};
use std::time::Duration;
use tokio::sync::broadcast::error::RecvError;

/// 录屏 JPEG 质量。
const SCREENCAST_QUALITY: u32 = 70;
/// 状态刷新的合并窗口，连续事件只触发一次汇总。
const REFRESH_DEBOUNCE: Duration = Duration::from_millis(120);

impl BrowserSession {
    /// 【内置浏览器】【事件泵】启动后台任务消费 CDP 事件；会话释放后自动退出。
    /// @returns 无
    pub(super) fn spawn_event_pump(self: &Arc<Self>) {
        let mut events = self.client.subscribe();
        let weak = Arc::downgrade(self);
        tokio::spawn(async move {
            loop {
                let event = match events.recv().await {
                    Ok(event) => event,
                    Err(RecvError::Lagged(_)) => continue,
                    Err(RecvError::Closed) => break,
                };
                let Some(session) = weak.upgrade() else {
                    break;
                };
                session.handle_event(event, &weak);
            }
        });
    }

    /// 【内置浏览器】【事件处理】按事件类型更新画面、标签与加载状态。
    /// @param event 为 CDP 事件；weak 为会话弱引用，供派生任务使用
    /// @returns 无
    fn handle_event(&self, event: CdpEvent, weak: &Weak<Self>) {
        match event.method.as_str() {
            // 1. 录屏帧：先确认再广播，确认慢了浏览器会停止推帧
            "Page.screencastFrame" => self.forward_frame(&event),
            // 2. 页面内新开窗口（target=_blank、window.open）时切换过去
            "Target.targetCreated" => {
                let info = event.params.get("targetInfo").cloned().unwrap_or_default();
                let is_popup = info.get("type").and_then(Value::as_str) == Some("page")
                    && info
                        .get("openerId")
                        .and_then(Value::as_str)
                        .is_some_and(|opener| !opener.is_empty());
                if is_popup {
                    let target = info
                        .get("targetId")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    spawn_with(weak, move |session| async move {
                        let _ = session.activate_target(&target).await;
                    });
                } else {
                    self.schedule_refresh(weak);
                }
            }
            // 3. 当前标签被页面自行关闭时，接替到剩余标签或新建空白页
            "Target.targetDestroyed" => {
                let target = event
                    .params
                    .get("targetId")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let was_active = {
                    let mut inner = self.inner.lock().unwrap();
                    inner.attached.remove(&target);
                    inner.active_target == target
                };
                if was_active {
                    spawn_with(weak, |session| async move {
                        session.recover_active_tab().await;
                    });
                } else {
                    self.schedule_refresh(weak);
                }
            }
            // 4. 主框架加载状态决定地址栏的加载指示
            "Page.frameStartedLoading" | "Page.frameStoppedLoading" => {
                if self.is_main_frame(&event) {
                    self.inner.lock().unwrap().loading = event.method == "Page.frameStartedLoading";
                    self.schedule_refresh(weak);
                }
            }
            // 主框架换了文档，隔离世界随之销毁，清掉缓存的上下文 ID
            "Page.frameNavigated" => {
                let is_main = event.params.pointer("/frame/parentId").is_none();
                if let (true, Some(session)) = (is_main, event.session_id.as_deref()) {
                    self.inner.lock().unwrap().isolated_contexts.remove(session);
                }
                self.schedule_refresh(weak);
            }
            "Target.targetInfoChanged" | "Page.navigatedWithinDocument" | "Page.loadEventFired" => {
                self.schedule_refresh(weak)
            }
            _ => {}
        }
    }

    /// 【内置浏览器】【主框架判断】判断事件是否来自当前标签的主框架。
    /// @param event 为框架加载事件
    /// @returns 主框架 ID 与目标 ID 相同，据此判断
    fn is_main_frame(&self, event: &CdpEvent) -> bool {
        let frame = event
            .params
            .get("frameId")
            .and_then(Value::as_str)
            .unwrap_or_default();
        self.inner.lock().unwrap().active_target == frame
    }

    /// 【内置浏览器】【画面转发】确认录屏帧并把 JPEG 广播给面板。
    /// @param event 为录屏帧事件
    /// @returns 无
    fn forward_frame(&self, event: &CdpEvent) {
        let session_id = event.session_id.as_deref();
        if let Some(ack) = event.params.get("sessionId").and_then(Value::as_i64) {
            self.client.fire(
                session_id,
                "Page.screencastFrameAck",
                json!({ "sessionId": ack }),
            );
        }
        let current = self.inner.lock().unwrap().screencast_session.clone();
        if session_id.is_none() || current.as_deref() != session_id {
            return;
        }
        let Some(data) = event.params.get("data").and_then(Value::as_str) else {
            return;
        };
        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(data) {
            let _ = self.events.send(BrowserEvent::Frame(Arc::new(bytes)));
        }
    }

    /// 【内置浏览器】【刷新合并】在合并窗口结束后汇总一次状态。
    /// @param weak 为会话弱引用
    /// @returns 无
    fn schedule_refresh(&self, weak: &Weak<Self>) {
        {
            let mut inner = self.inner.lock().unwrap();
            if inner.refresh_pending {
                return;
            }
            inner.refresh_pending = true;
        }
        spawn_with(weak, |session| async move {
            tokio::time::sleep(REFRESH_DEBOUNCE).await;
            session.inner.lock().unwrap().refresh_pending = false;
            session.refresh_state().await;
        });
    }

    /// 【内置浏览器】【标签接替】当前标签消失后切换到剩余标签，没有时新建空白页。
    /// @returns 无
    async fn recover_active_tab(&self) {
        let next = match self.page_targets().await {
            Ok(pages) => pages.into_iter().next().map(|page| page.target_id),
            Err(_) => return,
        };
        let next = match next {
            Some(next) => next,
            None => match self.create_target("about:blank").await {
                Ok(target) => target,
                Err(_) => return,
            },
        };
        let _ = self.activate_target(&next).await;
    }

    /// 【内置浏览器】【面板接入】登记一个面板连接，首个面板接入时开启录屏。
    /// @returns 无
    pub(crate) async fn attach_viewer(&self) {
        if self.viewers.fetch_add(1, Ordering::SeqCst) == 0 {
            self.restart_screencast().await;
        }
        // 静态页面开启录屏后可能很久不出新帧，补发一张截图作为首屏
        if let Ok((bytes, _)) = self.screenshot(false, None).await {
            let _ = self.events.send(BrowserEvent::Frame(Arc::new(bytes)));
        }
    }

    /// 【内置浏览器】【面板断开】注销面板连接，最后一个面板离开时停止录屏。
    /// @returns 无
    pub(crate) async fn detach_viewer(&self) {
        let previous = self.viewers.fetch_sub(1, Ordering::SeqCst);
        if previous <= 1 {
            self.viewers.store(0, Ordering::SeqCst);
            self.stop_screencast().await;
        }
    }

    /// 【内置浏览器】【录屏切换】在当前标签上重新开启录屏，没有面板时不开启。
    /// @returns 无
    pub(super) async fn restart_screencast(&self) {
        self.stop_screencast().await;
        if self.viewers.load(Ordering::SeqCst) == 0 {
            return;
        }
        let Ok(session) = self.active_session() else {
            return;
        };
        let (width, height) = self.viewport();
        let started = self
            .client
            .send(
                Some(&session),
                "Page.startScreencast",
                json!({
                    "format": "jpeg",
                    "quality": SCREENCAST_QUALITY,
                    "maxWidth": width,
                    "maxHeight": height,
                    "everyNthFrame": 1,
                }),
            )
            .await;
        if started.is_ok() {
            self.inner.lock().unwrap().screencast_session = Some(session);
        }
    }

    /// 【内置浏览器】【录屏停止】停止当前录屏。
    /// @returns 无
    async fn stop_screencast(&self) {
        let session = self.inner.lock().unwrap().screencast_session.take();
        if let Some(session) = session {
            let _ = self
                .client
                .send(Some(&session), "Page.stopScreencast", json!({}))
                .await;
        }
    }
}

/// 【内置浏览器】【派生任务】会话仍存活时在后台执行一段异步逻辑。
/// @param weak 为会话弱引用；task 为接收会话的异步闭包
/// @returns 无
fn spawn_with<F, Fut>(weak: &Weak<BrowserSession>, task: F)
where
    F: FnOnce(Arc<BrowserSession>) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let weak = weak.clone();
    tokio::spawn(async move {
        if let Some(session) = weak.upgrade() {
            task(session).await;
        }
    });
}
