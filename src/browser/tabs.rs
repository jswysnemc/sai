//! 标签页管理：列举、新建、切换、关闭与面板状态汇总。

use super::events::{short_tab_id, BrowserEvent, BrowserState, TabInfo};
use super::session::BrowserSession;
use anyhow::{bail, Result};
use serde_json::{json, Value};

/// 一个页面类目标。
#[derive(Clone, Debug)]
pub(super) struct PageTarget {
    pub(super) target_id: String,
    pub(super) title: String,
    pub(super) url: String,
}

impl BrowserSession {
    /// 【内置浏览器】【目标列举】列出全部页面类目标，忽略扩展与 Worker。
    /// @returns 页面目标列表，顺序与浏览器返回一致
    pub(super) async fn page_targets(&self) -> Result<Vec<PageTarget>> {
        let result = self
            .client
            .send(None, "Target.getTargets", json!({}))
            .await?;
        let targets = result
            .get("targetInfos")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        Ok(targets
            .iter()
            .filter(|info| info.get("type").and_then(Value::as_str) == Some("page"))
            .map(|info| PageTarget {
                target_id: string_field(info, "targetId"),
                title: string_field(info, "title"),
                url: string_field(info, "url"),
            })
            .collect())
    }

    /// 【内置浏览器】【目标新建】新建一个页面目标。
    /// @param url 为初始地址
    /// @returns 新目标 ID
    pub(super) async fn create_target(&self, url: &str) -> Result<String> {
        let result = self
            .client
            .send(None, "Target.createTarget", json!({ "url": url }))
            .await?;
        Ok(string_field(&result, "targetId"))
    }

    /// 【内置浏览器】【目标激活】附着到目标并设为当前标签页，同时切换录屏来源。
    /// @param target_id 为目标 ID
    /// @returns 操作结果
    pub(super) async fn activate_target(&self, target_id: &str) -> Result<()> {
        // 1. 首次激活时附着并开启页面事件，已附着的目标直接复用
        let known = self.inner.lock().unwrap().attached.get(target_id).cloned();
        let session = match known {
            Some(session) => session,
            None => {
                let result = self
                    .client
                    .send(
                        None,
                        "Target.attachToTarget",
                        json!({ "targetId": target_id, "flatten": true }),
                    )
                    .await?;
                let session = string_field(&result, "sessionId");
                self.client
                    .send(Some(&session), "Page.enable", json!({}))
                    .await?;
                self.inner
                    .lock()
                    .unwrap()
                    .attached
                    .insert(target_id.to_string(), session.clone());
                session
            }
        };
        // 2. 记录为当前标签，套用视口尺寸并把窗口切到前台
        self.inner.lock().unwrap().active_target = target_id.to_string();
        self.apply_viewport(&session).await?;
        let _ = self
            .client
            .send(Some(&session), "Page.bringToFront", json!({}))
            .await;
        // 3. 录屏跟随当前标签，并立即广播一次新状态
        self.restart_screencast().await;
        self.refresh_state().await;
        Ok(())
    }

    /// 【内置浏览器】【标签解析】把短标识或完整目标 ID 解析为目标 ID。
    /// @param id 为短标识前缀或完整 ID
    /// @returns 唯一匹配的目标 ID
    pub(super) async fn resolve_tab(&self, id: &str) -> Result<String> {
        let id = id.trim().to_lowercase();
        if id.is_empty() {
            bail!("tab id is required");
        }
        let matches: Vec<String> = self
            .page_targets()
            .await?
            .into_iter()
            .map(|target| target.target_id)
            .filter(|target| target.to_lowercase().starts_with(&id))
            .collect();
        match matches.as_slice() {
            [single] => Ok(single.clone()),
            [] => bail!("no browser tab matches id {id}; call browser with action=tabs to list tabs"),
            _ => bail!("tab id {id} is ambiguous; use more characters"),
        }
    }

    /// 【内置浏览器】【标签列举】返回带当前标记的标签列表。
    /// @returns 标签信息
    pub(crate) async fn list_tabs(&self) -> Result<Vec<TabInfo>> {
        let active = self.inner.lock().unwrap().active_target.clone();
        Ok(self
            .page_targets()
            .await?
            .into_iter()
            .map(|target| TabInfo {
                id: short_tab_id(&target.target_id),
                active: target.target_id == active,
                title: target.title,
                url: target.url,
            })
            .collect())
    }

    /// 【内置浏览器】【标签新建】新建标签页并切换过去。
    /// @param url 为初始地址，已经过地址策略校验
    /// @returns 新标签短标识
    pub(crate) async fn new_tab(&self, url: &str) -> Result<String> {
        let target = self.create_target(url).await?;
        self.activate_target(&target).await?;
        Ok(short_tab_id(&target))
    }

    /// 【内置浏览器】【标签切换】切换当前操作的标签页。
    /// @param id 为短标识
    /// @returns 操作结果
    pub(crate) async fn switch_tab(&self, id: &str) -> Result<()> {
        let target = self.resolve_tab(id).await?;
        self.activate_target(&target).await
    }

    /// 【内置浏览器】【标签关闭】关闭标签页；关闭最后一页时自动补一个空白页。
    /// @param id 为短标识，空表示当前标签
    /// @returns 操作结果
    pub(crate) async fn close_tab(&self, id: Option<&str>) -> Result<()> {
        // 1. 解析要关闭的目标，缺省为当前标签
        let target = match id.filter(|id| !id.trim().is_empty()) {
            Some(id) => self.resolve_tab(id).await?,
            None => self.inner.lock().unwrap().active_target.clone(),
        };
        // 2. 先确定关闭后的接替标签，避免当前标签悬空
        let remaining: Vec<String> = self
            .page_targets()
            .await?
            .into_iter()
            .map(|page| page.target_id)
            .filter(|page| *page != target)
            .collect();
        let next = match remaining.first() {
            Some(next) => next.clone(),
            None => self.create_target("about:blank").await?,
        };
        self.client
            .send(None, "Target.closeTarget", json!({ "targetId": target }))
            .await?;
        self.inner.lock().unwrap().attached.remove(&target);
        let was_active = self.inner.lock().unwrap().active_target == target;
        if was_active {
            self.activate_target(&next).await?;
        } else {
            self.refresh_state().await;
        }
        Ok(())
    }

    /// 【内置浏览器】【状态汇总】汇总标签、地址与历史状态。
    /// @returns 面板状态
    pub(crate) async fn current_state(&self) -> Result<BrowserState> {
        let tabs = self.list_tabs().await?;
        let (width, height, loading) = {
            let inner = self.inner.lock().unwrap();
            (inner.width, inner.height, inner.loading)
        };
        let active = tabs.iter().find(|tab| tab.active).cloned();
        let (can_go_back, can_go_forward) = self.history_flags().await.unwrap_or((false, false));
        Ok(BrowserState {
            url: active
                .as_ref()
                .map(|tab| tab.url.clone())
                .unwrap_or_default(),
            title: active.map(|tab| tab.title).unwrap_or_default(),
            tabs,
            loading,
            can_go_back,
            can_go_forward,
            width,
            height,
        })
    }

    /// 【内置浏览器】【状态广播】重新汇总状态，有变化时推送给面板。
    /// @returns 无
    pub(super) async fn refresh_state(&self) {
        let Ok(state) = self.current_state().await else {
            return;
        };
        {
            let mut inner = self.inner.lock().unwrap();
            if inner.last_state == state {
                return;
            }
            inner.last_state = state.clone();
        }
        let _ = self.events.send(BrowserEvent::State(state));
    }

    /// 【内置浏览器】【最近状态】返回最近一次广播的状态，供新连接的面板首屏使用。
    /// @returns 面板状态
    pub(crate) fn last_state(&self) -> BrowserState {
        self.inner.lock().unwrap().last_state.clone()
    }

    /// 【内置浏览器】【历史状态】读取当前标签的前进后退可用性。
    /// @returns (可后退, 可前进)
    async fn history_flags(&self) -> Result<(bool, bool)> {
        let history = self
            .page_send("Page.getNavigationHistory", json!({}))
            .await?;
        let index = history
            .get("currentIndex")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        let count = history
            .get("entries")
            .and_then(Value::as_array)
            .map_or(0, Vec::len) as i64;
        Ok((index > 0, index + 1 < count))
    }
}

/// 【内置浏览器】【字段读取】读取 JSON 对象中的字符串字段。
/// @param value 为 JSON 对象；key 为字段名
/// @returns 字段值，缺失时为空串
pub(super) fn string_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}
