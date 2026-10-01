//! 页面事件：alert/confirm/prompt 对话框、文件选择请求、下载进度与页面钩子安装。
//!
//! 无头浏览器不会绘制原生对话框；对话框不处理时页面脚本一直阻塞，
//! Agent 的后续操作也会卡住。面板在线时交给用户选择，否则按默认方式自动关闭。

use super::cdp::CdpEvent;
use super::events::{BrowserEvent, DialogInfo, FileChooserInfo};
use super::screencast::spawn_with;
use super::session::BrowserSession;
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::sync::atomic::Ordering;
use std::sync::Weak;
use std::time::Duration;

/// 面板在线时等待用户处理对话框的上限，超时后按默认方式关闭。
const DIALOG_TIMEOUT: Duration = Duration::from_secs(120);

impl BrowserSession {
    /// 【内置浏览器】【页面事件】分派对话框、文件选择、下载与文档就绪事件。
    /// @param event 为 CDP 事件；weak 为会话弱引用
    /// @returns 无
    pub(super) fn handle_page_event(&self, event: &CdpEvent, weak: &Weak<Self>) {
        match event.method.as_str() {
            "Page.javascriptDialogOpening" => self.dialog_opened(event, weak),
            "Page.javascriptDialogClosed" => {
                let closed = self.inner.lock().unwrap().dialog.take().is_some();
                if closed {
                    let _ = self.events.send(BrowserEvent::Dialog(None));
                }
            }
            "Page.fileChooserOpened" => self.file_chooser_opened(event),
            "Browser.downloadWillBegin" => self.download_began(&event.params),
            "Browser.downloadProgress" => self.download_progressed(&event.params),
            // 新文档就绪后重装下拉框拦截钩子，隔离世界随文档重建
            "Page.domContentEventFired" => {
                let Some(session) = event.session_id.clone() else {
                    return;
                };
                spawn_with(weak, move |browser| async move {
                    if browser.active_session().ok().as_deref() == Some(session.as_str()) {
                        let _ = browser.install_page_hooks().await;
                    }
                });
            }
            _ => {}
        }
    }

    /// 【内置浏览器】【对话框弹出】记录对话框；面板在线时交给用户，否则立即按默认方式关闭。
    /// @param event 为 Page.javascriptDialogOpening 事件；weak 为会话弱引用
    /// @returns 无
    fn dialog_opened(&self, event: &CdpEvent, weak: &Weak<Self>) {
        let Some(session) = event.session_id.clone() else {
            return;
        };
        let field = |key: &str| {
            event
                .params
                .get(key)
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string()
        };
        let info = DialogInfo {
            kind: field("type"),
            message: field("message"),
            default_prompt: field("defaultPrompt"),
            url: field("url"),
        };
        // 1. 没有面板时不等待：alert 与离开页面确认接受，confirm 与 prompt 取消
        if self.viewers.load(Ordering::SeqCst) == 0 {
            let accept = matches!(info.kind.as_str(), "alert" | "beforeunload");
            self.client.fire(
                Some(&session),
                "Page.handleJavaScriptDialog",
                json!({ "accept": accept }),
            );
            return;
        }
        // 2. 面板在线：广播给面板，超时仍未处理则按默认方式关闭
        self.inner.lock().unwrap().dialog = Some((session, info.clone()));
        let _ = self.events.send(BrowserEvent::Dialog(Some(info.clone())));
        spawn_with(weak, move |browser| async move {
            tokio::time::sleep(DIALOG_TIMEOUT).await;
            let still_open = browser
                .inner
                .lock()
                .unwrap()
                .dialog
                .as_ref()
                .is_some_and(|(_, open)| *open == info);
            if still_open {
                let accept = matches!(info.kind.as_str(), "alert" | "beforeunload");
                let _ = browser.answer_dialog(accept, None).await;
            }
        });
    }

    /// 【内置浏览器】【对话框回复】接受或取消当前对话框，prompt 可附带输入文本。
    /// @param accept 为是否确认；prompt_text 为 prompt 的输入
    /// @returns 操作结果；没有打开的对话框时报错
    pub(crate) async fn answer_dialog(
        &self,
        accept: bool,
        prompt_text: Option<&str>,
    ) -> Result<()> {
        let Some((session, _)) = self.inner.lock().unwrap().dialog.take() else {
            bail!("no dialog is open");
        };
        let mut params = json!({ "accept": accept });
        if let Some(text) = prompt_text {
            params["promptText"] = json!(text);
        }
        self.client
            .send(Some(&session), "Page.handleJavaScriptDialog", params)
            .await?;
        let _ = self.events.send(BrowserEvent::Dialog(None));
        Ok(())
    }

    /// 【内置浏览器】【当前对话框】返回仍在等待处理的对话框，供新连接的面板补显示。
    /// @returns 对话框信息
    pub(crate) fn pending_dialog(&self) -> Option<DialogInfo> {
        self.inner
            .lock()
            .unwrap()
            .dialog
            .as_ref()
            .map(|(_, info)| info.clone())
    }

    /// 【内置浏览器】【文件选择】记录等待文件的 input 节点并通知面板。
    /// @param event 为 Page.fileChooserOpened 事件
    /// @returns 无
    fn file_chooser_opened(&self, event: &CdpEvent) {
        let (Some(session), Some(node)) = (
            event.session_id.clone(),
            event.params.get("backendNodeId").and_then(Value::as_i64),
        ) else {
            return;
        };
        self.inner.lock().unwrap().file_chooser = Some((session, node));
        let info = FileChooserInfo {
            multiple: event.params.get("mode").and_then(Value::as_str) == Some("selectMultiple"),
            accept: String::new(),
        };
        let _ = self.events.send(BrowserEvent::FileChooser(Some(info)));
    }
}
