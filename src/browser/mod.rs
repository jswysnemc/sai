//! 内置浏览器：通过 Chrome DevTools Protocol 驱动本机 Chromium 系浏览器。
//!
//! 整个进程只维护一个浏览器会话，Agent 工具与 Web 工作台面板共用同一组标签页，
//! 因此面板里看到的页面就是 Agent 正在操作的页面。

mod actions;
mod cdp;
mod events;
mod isolated_world;
mod keys;
mod launcher;
pub(crate) mod navigation;
mod screencast;
mod session;
mod snapshot;
mod tabs;
mod url_policy;

#[cfg(test)]
mod tests;

pub(crate) use actions::{ClickKind, MouseInput, ScrollDirection};
pub(crate) use events::{BrowserEvent, BrowserState};
pub(crate) use keys::KeyInput;
pub(crate) use session::BrowserSession;
pub(crate) use snapshot::{DEFAULT_SNAPSHOT_CHARS, MAX_SNAPSHOT_CHARS};
pub(crate) use url_policy::normalize_url;

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;

/// 进程内唯一的浏览器会话槽位。
static BROWSER: Mutex<Option<Arc<BrowserSession>>> = Mutex::const_new(None);

/// 【内置浏览器】【会话获取】返回存活的浏览器会话，不存在或已退出时重新启动。
/// @returns 共享浏览器会话
pub(crate) async fn shared() -> Result<Arc<BrowserSession>> {
    let mut slot = BROWSER.lock().await;
    // 1. 已有会话仍然连通时直接复用
    if let Some(session) = slot.as_ref() {
        if session.is_alive() {
            return Ok(session.clone());
        }
    }
    // 2. 启动新的浏览器进程并附着到首个标签页
    let session = BrowserSession::launch().await?;
    *slot = Some(session.clone());
    Ok(session)
}

/// 【内置浏览器】【会话查询】返回当前已存在的会话，不会触发启动。
/// @returns 存活的会话；未启动时为空
pub(crate) async fn existing() -> Option<Arc<BrowserSession>> {
    BROWSER
        .lock()
        .await
        .as_ref()
        .filter(|session| session.is_alive())
        .cloned()
}

/// 【内置浏览器】【会话关闭】关闭浏览器进程并清空槽位，供服务退出时调用。
/// @returns 无
pub(crate) async fn shutdown() {
    let session = BROWSER.lock().await.take();
    if let Some(session) = session {
        session.close().await;
    }
}
