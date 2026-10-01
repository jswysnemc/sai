//! 浏览器会话的启动与关闭：修正无头标识、初始化页面能力、结束进程并清理临时目录。

use super::cdp::CdpClient;
use super::launcher::{self, LaunchOptions, LaunchedBrowser, DEFAULT_HEIGHT, DEFAULT_WIDTH};
use super::profile::{self, ProfileDir, ProfileMode};
use super::session::{BrowserSession, SessionInner};
use anyhow::Result;
use serde_json::{json, Value};
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::broadcast;

impl BrowserSession {
    /// 【内置浏览器】【会话启动】启动浏览器、建立连接并附着到首个页面。
    ///
    /// 无头模式首次启动时浏览器自报 `HeadlessChrome`；读取后改写为普通 Chrome
    /// 标识并缓存，再以该标识重新启动一次，之后的启动直接使用缓存。
    ///
    /// @param mode 为用户目录模式
    /// @returns 共享会话
    pub(crate) async fn launch_with(mode: ProfileMode) -> Result<Arc<Self>> {
        // 1. 启动浏览器；缓存的标识与当前可执行文件匹配时直接带上
        let executable = launcher::find_executable()?;
        let mut options = LaunchOptions {
            profile: mode,
            user_agent: profile::cached_user_agent(&executable),
        };
        let mut launched = launcher::launch(&options).await?;
        let mut client = CdpClient::connect(&launched.websocket_url).await?;
        // 2. 仍是无头标识时修正、缓存并重启
        if !launched.headed && options.user_agent.is_none() {
            let version = client.send(None, "Browser.getVersion", json!({})).await?;
            let reported = version
                .get("userAgent")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if let Some(user_agent) = profile::normal_user_agent(reported) {
                profile::store_user_agent(&launched.executable, &user_agent);
                shutdown_process(&client, &mut launched).await;
                options.user_agent = Some(user_agent);
                launched = launcher::launch(&options).await?;
                client = CdpClient::connect(&launched.websocket_url).await?;
            }
        }
        Self::start(client, launched).await
    }

    /// 【内置浏览器】【会话初始化】组装会话、开启目标发现并附着到首个页面。
    /// @param client 为已连接的 CDP 客户端；launched 为浏览器进程
    /// @returns 共享会话
    async fn start(client: CdpClient, launched: LaunchedBrowser) -> Result<Arc<Self>> {
        let (events, _) = broadcast::channel(64);
        let debug_port = debug_port(&launched.websocket_url);
        let persistent_profile = launched.profile.is_persistent();
        let session = Arc::new(Self {
            client,
            inner: Mutex::new(SessionInner {
                width: DEFAULT_WIDTH,
                height: DEFAULT_HEIGHT,
                scale: 1.0,
                ..SessionInner::default()
            }),
            events,
            viewers: AtomicUsize::new(0),
            process: tokio::sync::Mutex::new(Some(launched)),
            debug_port,
            persistent_profile,
        });
        // 1. 打开目标发现，标签页增删与标题变化都会推送事件
        session
            .client
            .send(
                None,
                "Target.setDiscoverTargets",
                json!({ "discover": true }),
            )
            .await?;
        // 2. 下载落到 Sai 管理的目录，页面无法弹出系统保存对话框
        session.configure_downloads().await;
        // 3. 事件泵先于附着启动，避免漏掉首个页面的加载事件
        session.spawn_event_pump();
        let target = match session.page_targets().await?.into_iter().next() {
            Some(target) => target.target_id,
            None => session.create_target("about:blank").await?,
        };
        session.activate_target(&target).await?;
        Ok(session)
    }

    /// 【内置浏览器】【进程关闭】请求浏览器正常退出，随后强制结束进程并清理临时目录。
    /// @returns 无
    pub(crate) async fn close(&self) {
        let Some(mut launched) = self.process.lock().await.take() else {
            return;
        };
        shutdown_process(&self.client, &mut launched).await;
        remove_temporary(launched.profile).await;
    }

    /// 【内置浏览器】【调试入口】返回 DevTools 前端地址，供本机打开调试工具。
    /// @returns 当前标签页的 DevTools 地址
    pub(crate) fn devtools_url(&self) -> Option<String> {
        let target = self.inner.lock().unwrap().active_target.clone();
        (self.debug_port > 0 && !target.is_empty()).then(|| {
            format!(
                "http://127.0.0.1:{port}/devtools/inspector.html?ws=127.0.0.1:{port}/devtools/page/{target}",
                port = self.debug_port
            )
        })
    }

    /// 【内置浏览器】【用户目录】是否使用持久用户目录。
    /// @returns 持久目录时为 true
    pub(crate) fn uses_persistent_profile(&self) -> bool {
        self.persistent_profile
    }
}

/// 【内置浏览器】【进程结束】请求浏览器退出，超时后强制结束。
/// @param client 为 CDP 客户端；launched 为浏览器进程
/// @returns 无
async fn shutdown_process(client: &CdpClient, launched: &mut LaunchedBrowser) {
    let _ = client
        .send_with_timeout(None, "Browser.close", json!({}), Duration::from_secs(2))
        .await;
    let exited = tokio::time::timeout(Duration::from_secs(3), launched.child.wait())
        .await
        .is_ok();
    if !exited {
        let _ = launched.child.kill().await;
    }
}

/// 【内置浏览器】【临时目录清理】删除临时用户目录；渲染进程可能仍在写入，失败时短暂重试。
/// @param profile 为用户目录；持久目录原样保留
/// @returns 无
async fn remove_temporary(profile: ProfileDir) {
    let ProfileDir::Temporary(dir) = profile else {
        return;
    };
    let path = dir.keep();
    for _ in 0..10 {
        if tokio::fs::remove_dir_all(&path).await.is_ok() || !path.exists() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// 【内置浏览器】【端口解析】从浏览器级调试地址中取出端口。
/// @param websocket_url 为 `ws://127.0.0.1:<port>/devtools/browser/<id>`
/// @returns 端口；解析失败时为 0
pub(super) fn debug_port(websocket_url: &str) -> u16 {
    websocket_url
        .strip_prefix("ws://127.0.0.1:")
        .and_then(|rest| rest.split('/').next())
        .and_then(|port| port.parse().ok())
        .unwrap_or(0)
}
