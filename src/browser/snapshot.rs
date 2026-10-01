//! 页面结构快照与 ref 定位。

use super::session::BrowserSession;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::Value;

/// 快照注入脚本，参数占位符在运行时替换。
const SNAPSHOT_SCRIPT: &str = include_str!("snapshot_script.js");
/// 快照默认字符上限。
pub(crate) const DEFAULT_SNAPSHOT_CHARS: usize = 20_000;
/// 快照字符上限的最大可调值。
pub(crate) const MAX_SNAPSHOT_CHARS: usize = 80_000;

/// 一次快照的结果。
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PageSnapshot {
    pub(crate) title: String,
    pub(crate) url: String,
    pub(crate) text: String,
    pub(crate) refs: usize,
    pub(crate) truncated: bool,
    pub(crate) scroll_y: i64,
    pub(crate) scroll_height: i64,
    pub(crate) viewport_height: i64,
}

impl PageSnapshot {
    /// 【内置浏览器】【快照文本】生成交给模型的快照全文。
    /// @returns 头部概况加结构树文本
    pub(crate) fn render(&self) -> String {
        let mut output = format!(
            "Page: {}\nURL: {}\nScroll: {}/{} px (viewport {} px)\nRefs: {}\n",
            self.title,
            self.url,
            self.scroll_y,
            self.scroll_height,
            self.viewport_height,
            self.refs
        );
        output.push_str("\n");
        if self.text.is_empty() {
            output.push_str("(page has no visible content)");
        } else {
            output.push_str(&self.text);
        }
        if self.truncated {
            output.push_str(
                "\n\n[snapshot truncated: raise max_chars, use interactive_only, or scroll and snapshot again]",
            );
        }
        output
    }
}

/// ref 定位后的元素几何信息。
#[derive(Clone, Debug, Deserialize)]
pub(super) struct ElementBox {
    /// 元素中心横坐标（顶层视口 CSS 像素）
    pub(super) x: f64,
    /// 元素中心纵坐标
    pub(super) y: f64,
    pub(super) width: f64,
    pub(super) height: f64,
    /// 元素中心被其他元素覆盖
    #[serde(default)]
    pub(super) covered: bool,
    /// 元素标签名，供结果说明
    #[serde(default)]
    pub(super) tag: String,
}

/// 【内置浏览器】【快照脚本】替换占位符生成可执行的快照脚本。
/// @param max_chars 为字符上限；interactive_only 为是否只列可交互元素
/// @returns 脚本源码
pub(super) fn snapshot_script(max_chars: usize, interactive_only: bool) -> String {
    SNAPSHOT_SCRIPT
        .replace("__MAX_CHARS__", &max_chars.to_string())
        .replace("__INTERACTIVE_ONLY__", &interactive_only.to_string())
}

/// 【内置浏览器】【ref 表达式】生成按 ref 取元素的表达式，元素失效时抛出可读错误。
/// @param reference 为快照中的 ref，如 `e12`
/// @returns 返回元素的 JavaScript 表达式
pub(super) fn element_expression(reference: &str) -> Result<String> {
    let reference = reference.trim().trim_start_matches("ref=");
    if reference.len() < 2
        || !reference.starts_with('e')
        || !reference[1..].chars().all(|c| c.is_ascii_digit())
    {
        bail!("invalid ref {reference:?}; use a ref like e12 from browser_snapshot");
    }
    Ok(format!(
        "(() => {{ const el = globalThis.__saiRefs && globalThis.__saiRefs.get({ref}); \
         if (!el) throw new Error('ref {raw} not found; take a new browser_snapshot'); \
         if (!el.isConnected) throw new Error('ref {raw} is no longer on the page; take a new browser_snapshot'); \
         return el; }})()",
        ref = serde_json::to_string(reference)?,
        raw = reference
    ))
}

impl BrowserSession {
    /// 【内置浏览器】【结构快照】提取当前页面的无障碍结构树并刷新 ref 表。
    /// @param max_chars 为字符上限；interactive_only 为是否只列可交互元素
    /// @returns 快照结果
    pub(crate) async fn snapshot(
        &self,
        max_chars: usize,
        interactive_only: bool,
    ) -> Result<PageSnapshot> {
        let max_chars = max_chars.clamp(2_000, MAX_SNAPSHOT_CHARS);
        let value = self
            .evaluate_isolated(&snapshot_script(max_chars, interactive_only))
            .await?;
        serde_json::from_value(value).context("parse page snapshot")
    }

    /// 【内置浏览器】【元素定位】按 ref 找回元素，滚动到视口内并返回中心坐标。
    /// @param reference 为快照 ref
    /// @returns 元素几何信息
    pub(super) async fn locate(&self, reference: &str) -> Result<ElementBox> {
        let element = element_expression(reference)?;
        let script = format!(
            "(() => {{ const el = {element}; \
             el.scrollIntoView({{ block: 'center', inline: 'center', behavior: 'instant' }}); \
             const rect = el.getBoundingClientRect(); \
             let x = rect.left + rect.width / 2, y = rect.top + rect.height / 2; \
             let win = el.ownerDocument.defaultView, framed = false; \
             while (win && win.frameElement) {{ const frame = win.frameElement.getBoundingClientRect(); \
               x += frame.left; y += frame.top; win = win.parent; framed = true; }} \
             let covered = false; \
             if (!framed) {{ const hit = document.elementFromPoint(x, y); \
               covered = !!hit && hit !== el && !el.contains(hit) && !hit.contains(el); }} \
             return {{ x, y, width: rect.width, height: rect.height, covered, tag: el.tagName.toLowerCase() }}; }})()"
        );
        let value = self.evaluate_isolated(&script).await?;
        let element: ElementBox = serde_json::from_value(value).context("parse element box")?;
        if element.width <= 0.0 || element.height <= 0.0 {
            bail!("ref {reference} has no visible size; it may be hidden or collapsed");
        }
        Ok(element)
    }

    /// 【内置浏览器】【元素脚本】以 ref 对应元素为 `el` 执行一段函数体。
    /// @param reference 为快照 ref；body 为以 `el` 为参数的函数体
    /// @returns 函数返回值
    pub(super) async fn with_element(&self, reference: &str, body: &str) -> Result<Value> {
        let element = element_expression(reference)?;
        self.evaluate_isolated(&format!("((el) => {{ {body} }})({element})"))
            .await
    }
}
