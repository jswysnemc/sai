//! 面板交互补充：原生下拉框选项、复制到本机剪贴板、元素选择与文件上传。

use super::events::{BrowserEvent, SelectPopup};
use super::keys::KeyInput;
use super::session::BrowserSession;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::time::Duration;

/// 页面钩子脚本：拦截下拉框、读取选区。
const PAGE_HOOKS_SCRIPT: &str = include_str!("page_hooks.js");
/// 元素选择脚本，信息卡标签占位符在运行时替换。
const PICKER_SCRIPT: &str = include_str!("picker_script.js");
/// 等待用户在页面上选中元素的上限。
const PICK_TIMEOUT: Duration = Duration::from_secs(600);

impl BrowserSession {
    /// 【内置浏览器】【页面钩子】在当前页面的隔离世界中安装下拉框与选区钩子。
    /// @returns 操作结果；重复安装时直接返回
    pub(super) async fn install_page_hooks(&self) -> Result<()> {
        self.evaluate_isolated(PAGE_HOOKS_SCRIPT).await?;
        Ok(())
    }

    /// 【内置浏览器】【面板鼠标】按下后检查是否点开了原生下拉框，有则交给面板绘制选项。
    /// @returns 无；钩子未安装时补装，供下一次点击生效
    pub(crate) async fn take_select_popup(&self) {
        let value = self
            .evaluate_isolated(
                "globalThis.__saiPageHooks ? globalThis.__saiPageHooks.takeSelect() : 'missing'",
            )
            .await;
        match value {
            Ok(Value::String(flag)) if flag == "missing" => {
                let _ = self.install_page_hooks().await;
            }
            Ok(value @ Value::Object(_)) => {
                if let Ok(popup) = serde_json::from_value::<SelectPopupWire>(value) {
                    let _ = self.events.send(BrowserEvent::SelectPopup(popup.into()));
                }
            }
            _ => {}
        }
    }

    /// 【内置浏览器】【下拉选择】应用面板中选中的下拉框选项。
    /// @param index 为选项下标
    /// @returns 操作结果
    pub(crate) async fn apply_select(&self, index: i64) -> Result<()> {
        self.evaluate_isolated(&format!(
            "(() => {{ if (!globalThis.__saiPageHooks) throw new Error('the page changed; open the menu again'); \
             return globalThis.__saiPageHooks.applySelect({index}); }})()"
        ))
        .await?;
        Ok(())
    }

    /// 【内置浏览器】【面板按键】转发按键；复制或剪切时先读出选中文本交给本机剪贴板。
    /// @param input 为面板键盘事件
    /// @returns 操作结果
    pub(crate) async fn dispatch_panel_key(&self, input: &KeyInput) -> Result<()> {
        // 剪切会清掉选区，必须在按键转发前读取
        let copying = input.kind == "down"
            && input.modifiers & (2 | 4) != 0
            && matches!(input.key.to_ascii_lowercase().as_str(), "c" | "x");
        let copied = if copying {
            self.evaluate_isolated(&format!(
                "({PAGE_HOOKS_SCRIPT}) && globalThis.__saiPageHooks.selection()"
            ))
            .await
            .ok()
            .and_then(|value| value.as_str().map(str::to_string))
            .filter(|text| !text.is_empty())
        } else {
            None
        };
        self.dispatch_key_input(input).await?;
        if let Some(text) = copied {
            let _ = self.events.send(BrowserEvent::Clipboard(text));
        }
        Ok(())
    }

    /// 【内置浏览器】【元素选择】在页面上高亮鼠标下的元素，等待用户点击选中。
    /// @param labels 为信息卡标签：背景、颜色、字体
    /// @returns 选中时为元素信息，取消或页面跳转时为空
    pub(crate) async fn pick_element(&self, labels: &[&str; 3]) -> Result<Option<Value>> {
        let labels = json!({ "background": labels[0], "color": labels[1], "font": labels[2] });
        let script = PICKER_SCRIPT.replace("__LABELS__", &labels.to_string());
        let session = self.active_session()?;
        // 1. 选择可能持续很久，单独使用较长的等待时间
        let context = self.isolated_context().await?;
        let result = self
            .client
            .send_with_timeout(
                Some(&session),
                "Runtime.evaluate",
                json!({
                    "expression": script,
                    "contextId": context,
                    "returnByValue": true,
                    "awaitPromise": true,
                }),
                PICK_TIMEOUT,
            )
            .await;
        // 2. 页面跳转会销毁隔离世界，按取消处理
        let result = match result {
            Ok(result) => result,
            Err(error) if format!("{error:#}").contains("context") => return Ok(None),
            Err(error) => return Err(error),
        };
        if let Some(details) = result.get("exceptionDetails") {
            let text = details
                .pointer("/exception/description")
                .and_then(Value::as_str)
                .unwrap_or("element picker failed");
            bail!("{}", text.lines().next().unwrap_or(text));
        }
        let value = result
            .pointer("/result/value")
            .cloned()
            .unwrap_or(Value::Null);
        if value.get("status").and_then(Value::as_str) == Some("selected") {
            return Ok(value.get("element").cloned());
        }
        Ok(None)
    }

    /// 【内置浏览器】【元素选择】取消正在进行的元素选择。
    /// @returns 操作结果
    pub(crate) async fn cancel_pick(&self) -> Result<()> {
        self.evaluate_isolated("globalThis.__saiElementPicker?.cancel?.(); true")
            .await
            .context("cancel element picker")?;
        Ok(())
    }
}

/// 页面钩子返回的下拉框数据。
#[derive(serde::Deserialize)]
struct SelectPopupWire {
    options: Vec<super::events::SelectOption>,
    selected: i64,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl From<SelectPopupWire> for SelectPopup {
    /// 【内置浏览器】【下拉框数据】转换为面板事件。
    /// @param wire 为页面钩子数据
    /// @returns 面板事件数据
    fn from(wire: SelectPopupWire) -> Self {
        Self {
            options: wire.options,
            selected: wire.selected,
            x: wire.x,
            y: wire.y,
            width: wire.width,
            height: wire.height,
        }
    }
}
