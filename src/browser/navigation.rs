//! 页面导航：打开地址、前进后退、刷新与等待页面就绪。

use super::session::BrowserSession;
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::time::Duration;

/// 导航后等待页面加载完成的上限。
pub(crate) const LOAD_TIMEOUT: Duration = Duration::from_secs(15);
/// 交互操作后等待可能触发的导航稳定下来的上限。
pub(crate) const SETTLE_TIMEOUT: Duration = Duration::from_secs(5);

/// 历史记录方向。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HistoryStep {
    Back,
    Forward,
}

/// 导航完成后的页面概况。
#[derive(Clone, Debug)]
pub(crate) struct PageSummary {
    pub(crate) url: String,
    pub(crate) title: String,
    /// 等待超时仍未加载完成
    pub(crate) still_loading: bool,
}

impl PageSummary {
    /// 【内置浏览器】【概况文本】生成给模型看的一行页面概况。
    /// @returns 包含标题、地址与加载状态的文本
    pub(crate) fn describe(&self) -> String {
        let mut text = format!("Page: {}\nURL: {}", self.title, self.url);
        if self.still_loading {
            text.push_str(
                "\nNote: page is still loading; take a snapshot again if content looks incomplete.",
            );
        }
        text
    }
}

impl BrowserSession {
    /// 【内置浏览器】【地址导航】在当前标签打开地址并等待加载完成。
    /// @param url 为已通过地址策略的完整地址
    /// @returns 页面概况
    pub(crate) async fn navigate(&self, url: &str) -> Result<PageSummary> {
        // 1. Page.navigate 在导航提交或失败后返回，网络错误通过 errorText 告知
        let result = self
            .page_send("Page.navigate", json!({ "url": url }))
            .await?;
        if let Some(error) = result
            .get("errorText")
            .and_then(Value::as_str)
            .filter(|text| !text.is_empty())
        {
            bail!("navigation to {url} failed: {error}");
        }
        // 2. 等待新文档加载完成
        self.wait_until_ready(LOAD_TIMEOUT).await
    }

    /// 【内置浏览器】【历史导航】在历史记录中后退或前进一步。
    /// @param step 为方向
    /// @returns 页面概况
    pub(crate) async fn go_history(&self, step: HistoryStep) -> Result<PageSummary> {
        let history = self
            .page_send("Page.getNavigationHistory", json!({}))
            .await?;
        let index = history
            .get("currentIndex")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let entries = history
            .get("entries")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let target = match step {
            HistoryStep::Back => index - 1,
            HistoryStep::Forward => index + 1,
        };
        let Some(entry) = usize::try_from(target)
            .ok()
            .and_then(|target| entries.get(target))
        else {
            let direction = match step {
                HistoryStep::Back => "back",
                HistoryStep::Forward => "forward",
            };
            bail!("no history entry to go {direction}");
        };
        let entry_id = entry.get("id").cloned().unwrap_or(Value::Null);
        self.page_send(
            "Page.navigateToHistoryEntry",
            json!({ "entryId": entry_id }),
        )
        .await?;
        self.settle(LOAD_TIMEOUT).await
    }

    /// 【内置浏览器】【页面刷新】重新加载当前页面。
    /// @returns 页面概况
    pub(crate) async fn reload(&self) -> Result<PageSummary> {
        self.page_send("Page.reload", json!({})).await?;
        self.settle(LOAD_TIMEOUT).await
    }

    /// 【内置浏览器】【停止加载】停止当前页面的加载。
    /// @returns 操作结果
    pub(crate) async fn stop_loading(&self) -> Result<()> {
        self.page_send("Page.stopLoading", json!({})).await?;
        Ok(())
    }

    /// 【内置浏览器】【操作稳定】交互或异步导航后短暂等待，再等待文档就绪。
    /// @param timeout 为等待上限
    /// @returns 页面概况
    pub(crate) async fn settle(&self, timeout: Duration) -> Result<PageSummary> {
        // 不等待提交的命令需要留出时间让新导航开始，否则读到的还是旧文档状态
        tokio::time::sleep(Duration::from_millis(300)).await;
        self.wait_until_ready(timeout).await
    }

    /// 【内置浏览器】【就绪等待】轮询 document.readyState，超时不报错只标记仍在加载。
    /// @param timeout 为等待上限
    /// @returns 页面概况
    pub(crate) async fn wait_until_ready(&self, timeout: Duration) -> Result<PageSummary> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            // 导航过程中执行上下文可能被销毁，单次求值失败继续重试
            let state = self
                .evaluate("[document.readyState, location.href, document.title]")
                .await
                .ok();
            if let Some(Value::Array(values)) = state {
                let text = |index: usize| {
                    values
                        .get(index)
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string()
                };
                let ready = text(0) == "complete";
                if ready || tokio::time::Instant::now() >= deadline {
                    self.refresh_state().await;
                    return Ok(PageSummary {
                        url: text(1),
                        title: text(2),
                        still_loading: !ready,
                    });
                }
            } else if tokio::time::Instant::now() >= deadline {
                bail!("page did not respond within {timeout:?}");
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    /// 【内置浏览器】【条件等待】等待页面出现指定文本或选择器。
    /// @param text 为待出现的文本；selector 为 CSS 选择器；timeout 为等待上限
    /// @returns 满足条件时的页面概况；超时报错
    pub(crate) async fn wait_for(
        &self,
        text: Option<&str>,
        selector: Option<&str>,
        timeout: Duration,
    ) -> Result<PageSummary> {
        let condition = match (text, selector) {
            (Some(text), _) => format!(
                "(document.body?.innerText ?? '').includes({})",
                serde_json::to_string(text)?
            ),
            (None, Some(selector)) => format!(
                "(() => {{ try {{ return !!document.querySelector({}); }} catch (_) {{ return 'invalid-selector'; }} }})()",
                serde_json::to_string(selector)?
            ),
            (None, None) => {
                tokio::time::sleep(timeout).await;
                return self.wait_until_ready(SETTLE_TIMEOUT).await;
            }
        };
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            match self.evaluate(&condition).await.ok() {
                Some(Value::Bool(true)) => return self.wait_until_ready(SETTLE_TIMEOUT).await,
                Some(Value::String(flag)) if flag == "invalid-selector" => {
                    bail!("invalid CSS selector: {}", selector.unwrap_or_default())
                }
                _ => {}
            }
            if tokio::time::Instant::now() >= deadline {
                bail!("condition not met within {timeout:?}");
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
        }
    }
}
