//! Sai 专用隔离世界：快照与 ref 定位脚本在独立的 JavaScript 全局对象中运行。
//!
//! 隔离世界与页面共享 DOM，但不共享 `window` 上的变量。ref 表因此不会被页面脚本
//! 读取或替换，恶意页面无法把模型的点击重定向到别的元素。

use super::session::BrowserSession;
use anyhow::{Context, Result};
use serde_json::{json, Value};

/// 隔离世界名称，便于在 DevTools 中辨认。
const WORLD_NAME: &str = "sai-browser";

impl BrowserSession {
    /// 【内置浏览器】【隔离求值】在当前标签的 Sai 隔离世界中执行表达式。
    ///
    /// 页面导航会销毁旧上下文；遇到上下文失效时重建一次再执行。
    /// 新上下文中没有旧 ref 表，按 ref 定位的脚本会报出需要重新快照的错误。
    ///
    /// @param expression 为 JavaScript 表达式
    /// @returns 表达式结果
    pub(super) async fn evaluate_isolated(&self, expression: &str) -> Result<Value> {
        let session = self.active_session()?;
        // 1. 先用缓存的上下文执行
        let cached = self
            .inner
            .lock()
            .unwrap()
            .isolated_contexts
            .get(&session)
            .copied();
        if let Some(context) = cached {
            match self.evaluate_in(&session, Some(context), expression).await {
                Ok(value) => return Ok(value),
                Err(error) if is_stale_context(&error) => {}
                Err(error) => return Err(error),
            }
        }
        // 2. 上下文不存在或已随导航销毁时新建
        let context = self.create_isolated_world(&session).await?;
        self.evaluate_in(&session, Some(context), expression).await
    }

    /// 【内置浏览器】【隔离上下文】返回当前标签的隔离世界上下文，不存在时新建。
    ///
    /// 只用于需要自定义等待时间的长时间求值；缓存的上下文可能已随导航失效，
    /// 调用方需把上下文错误按页面已跳转处理。
    ///
    /// @returns 执行上下文 ID
    pub(super) async fn isolated_context(&self) -> Result<i64> {
        let session = self.active_session()?;
        let cached = self
            .inner
            .lock()
            .unwrap()
            .isolated_contexts
            .get(&session)
            .copied();
        match cached {
            Some(context) => Ok(context),
            None => self.create_isolated_world(&session).await,
        }
    }

    /// 【内置浏览器】【隔离世界创建】在主框架创建隔离世界并缓存上下文 ID。
    /// @param session 为 sessionId
    /// @returns 执行上下文 ID
    async fn create_isolated_world(&self, session: &str) -> Result<i64> {
        let tree = self
            .client
            .send(Some(session), "Page.getFrameTree", json!({}))
            .await?;
        let frame_id = tree
            .pointer("/frameTree/frame/id")
            .and_then(Value::as_str)
            .context("page has no main frame")?;
        let result = self
            .client
            .send(
                Some(session),
                "Page.createIsolatedWorld",
                json!({ "frameId": frame_id, "worldName": WORLD_NAME }),
            )
            .await?;
        let context = result
            .get("executionContextId")
            .and_then(Value::as_i64)
            .context("isolated world returned no context id")?;
        self.inner
            .lock()
            .unwrap()
            .isolated_contexts
            .insert(session.to_string(), context);
        Ok(context)
    }
}

/// 【内置浏览器】【上下文失效】判断错误是否因执行上下文已被导航销毁。
/// @param error 为求值错误
/// @returns 上下文失效时为 true
fn is_stale_context(error: &anyhow::Error) -> bool {
    let text = format!("{error:#}");
    text.contains("Cannot find context") || text.contains("context was destroyed")
}
