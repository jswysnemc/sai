//! 页面交互：点击、悬停、输入、下拉选择、滚动、截图与面板鼠标转发。

use super::navigation::{PageSummary, SETTLE_TIMEOUT};
use super::session::BrowserSession;
use anyhow::{bail, Context, Result};
use base64::Engine;
use serde::Deserialize;
use serde_json::{json, Value};

/// 整页截图的最大高度，避免超长页面生成巨幅图片。
const FULL_PAGE_MAX_HEIGHT: f64 = 8_000.0;

/// 点击方式。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClickKind {
    Single,
    Double,
    Right,
}

/// 滚动方向。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

impl ScrollDirection {
    /// 【内置浏览器】【方向解析】解析工具参数中的滚动方向。
    /// @param value 为 `up`、`down`、`left` 或 `right`
    /// @returns 滚动方向
    pub(crate) fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "up" => Ok(Self::Up),
            "down" => Ok(Self::Down),
            "left" => Ok(Self::Left),
            "right" => Ok(Self::Right),
            other => bail!("unknown scroll direction: {other}"),
        }
    }

    /// 【内置浏览器】【方向名称】返回小写方向名。
    /// @returns 方向名
    fn label(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
            Self::Left => "left",
            Self::Right => "right",
        }
    }
}

/// 面板转发的一次鼠标事件，坐标为页面 CSS 像素。
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct MouseInput {
    /// `move`、`down`、`up` 或 `wheel`
    pub(crate) kind: String,
    pub(crate) x: f64,
    pub(crate) y: f64,
    #[serde(default)]
    pub(crate) button: Option<String>,
    #[serde(default)]
    pub(crate) click_count: Option<i64>,
    #[serde(default)]
    pub(crate) delta_x: f64,
    #[serde(default)]
    pub(crate) delta_y: f64,
    #[serde(default)]
    pub(crate) modifiers: i64,
}

/// 一次交互操作的结果。
#[derive(Clone, Debug)]
pub(crate) struct ActionOutcome {
    /// 操作说明，如 `clicked button`
    pub(crate) message: String,
    /// 操作后的页面概况
    pub(crate) page: PageSummary,
}

impl ActionOutcome {
    /// 【内置浏览器】【结果文本】生成交给模型的操作结果。
    /// @returns 操作说明与页面概况
    pub(crate) fn describe(&self) -> String {
        format!(
            "{}\n{}\nCall browser with action=snapshot to see the updated page.",
            self.message,
            self.page.describe()
        )
    }
}

impl BrowserSession {
    /// 【内置浏览器】【元素点击】按 ref 定位元素并以真实鼠标事件点击。
    /// @param reference 为快照 ref；kind 为点击方式
    /// @returns 操作结果
    pub(crate) async fn click(&self, reference: &str, kind: ClickKind) -> Result<ActionOutcome> {
        // 1. 定位元素中心并移动鼠标过去，触发悬停类交互
        let element = self.locate(reference).await?;
        self.mouse_event("mouseMoved", element.x, element.y, "none", 0, 0)
            .await?;
        // 2. 按点击方式发送按下与抬起
        let (button, count) = match kind {
            ClickKind::Single => ("left", 1),
            ClickKind::Double => ("left", 2),
            ClickKind::Right => ("right", 1),
        };
        for click in 1..=count {
            self.mouse_event("mousePressed", element.x, element.y, button, click, 0)
                .await?;
            self.mouse_event("mouseReleased", element.x, element.y, button, click, 0)
                .await?;
        }
        // 3. 点击可能触发导航，等待页面稳定后返回
        let mut message = format!("Clicked {} [ref={reference}]", element.tag);
        if element.covered {
            message.push_str(
                " (warning: another element covered its center; the click may have hit an overlay)",
            );
        }
        let page = self.settle(SETTLE_TIMEOUT).await?;
        Ok(ActionOutcome { message, page })
    }

    /// 【内置浏览器】【元素悬停】把鼠标移动到元素上方。
    /// @param reference 为快照 ref
    /// @returns 操作结果
    pub(crate) async fn hover(&self, reference: &str) -> Result<ActionOutcome> {
        let element = self.locate(reference).await?;
        self.mouse_event("mouseMoved", element.x, element.y, "none", 0, 0)
            .await?;
        let page = self.settle(SETTLE_TIMEOUT).await?;
        Ok(ActionOutcome {
            message: format!("Hovered {} [ref={reference}]", element.tag),
            page,
        })
    }

    /// 【内置浏览器】【文本输入】聚焦元素，可选清空原内容后输入文本。
    /// @param reference 为快照 ref；text 为输入内容；clear 为是否先清空；submit 为输入后是否按回车
    /// @returns 操作结果
    pub(crate) async fn type_text(
        &self,
        reference: &str,
        text: &str,
        clear: bool,
        submit: bool,
    ) -> Result<ActionOutcome> {
        // 1. 点击聚焦，兼容需要点击才激活的自定义输入框
        let element = self.locate(reference).await?;
        self.mouse_event("mousePressed", element.x, element.y, "left", 1, 0)
            .await?;
        self.mouse_event("mouseReleased", element.x, element.y, "left", 1, 0)
            .await?;
        // 2. 需要清空时选中全部原内容，随后的输入会直接替换
        let focus_body = if clear {
            "el.focus(); \
             if (typeof el.select === 'function' && 'value' in el) { el.select(); } \
             else if (el.isContentEditable) { const range = document.createRange(); range.selectNodeContents(el); \
               const selection = getSelection(); selection.removeAllRanges(); selection.addRange(range); } \
             return true;"
        } else {
            "el.focus(); return true;"
        };
        self.with_element(reference, focus_body).await?;
        // 3. 清空且新内容为空时用退格删除选区，否则插入文本
        if text.is_empty() {
            if clear {
                self.press_key("Backspace").await?;
            }
        } else {
            self.insert_text(text).await?;
        }
        if submit {
            self.press_key("Enter").await?;
        }
        let page = self.settle(SETTLE_TIMEOUT).await?;
        Ok(ActionOutcome {
            message: format!(
                "Typed {} characters into {} [ref={reference}]{}",
                text.chars().count(),
                element.tag,
                if submit { " and pressed Enter" } else { "" }
            ),
            page,
        })
    }

    /// 【内置浏览器】【下拉选择】按选项文本或值选中原生下拉框的选项。
    /// @param reference 为快照 ref；option 为选项文本或 value
    /// @returns 操作结果
    pub(crate) async fn select_option(
        &self,
        reference: &str,
        option: &str,
    ) -> Result<ActionOutcome> {
        let body = format!(
            "if (el.tagName !== 'SELECT') throw new Error('ref is not a <select>; click it and pick an option instead'); \
             const wanted = {wanted}; \
             const match = [...el.options].find((o) => o.value === wanted || o.text.trim() === wanted) \
               || [...el.options].find((o) => o.text.toLowerCase().includes(wanted.toLowerCase())); \
             if (!match) throw new Error('no option matches ' + wanted); \
             el.value = match.value; \
             el.dispatchEvent(new Event('input', {{ bubbles: true }})); \
             el.dispatchEvent(new Event('change', {{ bubbles: true }})); \
             return match.text.trim();",
            wanted = serde_json::to_string(option)?
        );
        let selected = self.with_element(reference, &body).await?;
        let page = self.settle(SETTLE_TIMEOUT).await?;
        Ok(ActionOutcome {
            message: format!(
                "Selected option {} in [ref={reference}]",
                selected.as_str().unwrap_or(option)
            ),
            page,
        })
    }

    /// 【内置浏览器】【页面滚动】在页面或指定元素上方发送滚轮事件。
    /// @param direction 为方向；amount 为像素距离；reference 为可选元素 ref
    /// @returns 滚动后的位置说明
    pub(crate) async fn scroll(
        &self,
        direction: ScrollDirection,
        amount: f64,
        reference: Option<&str>,
    ) -> Result<String> {
        // 1. 有 ref 时在元素中心滚动，以便滚动内部可滚动容器；否则在视口中心滚动
        let (x, y) = match reference {
            Some(reference) => {
                let element = self.locate(reference).await?;
                (element.x, element.y)
            }
            None => {
                let (width, height) = self.viewport();
                (f64::from(width) / 2.0, f64::from(height) / 2.0)
            }
        };
        let (delta_x, delta_y) = match direction {
            ScrollDirection::Up => (0.0, -amount),
            ScrollDirection::Down => (0.0, amount),
            ScrollDirection::Left => (-amount, 0.0),
            ScrollDirection::Right => (amount, 0.0),
        };
        self.page_send(
            "Input.dispatchMouseEvent",
            json!({ "type": "mouseWheel", "x": x, "y": y, "deltaX": delta_x, "deltaY": delta_y }),
        )
        .await?;
        // 2. 滚动是异步完成的，稍等后读取位置
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        let position = self
            .evaluate(
                "(() => { const s = document.scrollingElement || document.documentElement; \
                 return [Math.round(s.scrollTop), Math.round(s.scrollHeight), innerHeight, Math.round(s.scrollLeft)]; })()",
            )
            .await?;
        let number = |index: usize| position.get(index).and_then(Value::as_i64).unwrap_or(0);
        Ok(format!(
            "Scrolled {} by {amount} px. Page scroll: top {} / height {} (viewport {}), left {}.",
            direction.label(),
            number(0),
            number(1),
            number(2),
            number(3)
        ))
    }

    /// 【内置浏览器】【元素滚入】把元素滚动到视口中央。
    /// @param reference 为快照 ref
    /// @returns 结果说明
    pub(crate) async fn scroll_into_view(&self, reference: &str) -> Result<String> {
        let element = self.locate(reference).await?;
        Ok(format!(
            "Scrolled {} [ref={reference}] into view at ({:.0}, {:.0}).",
            element.tag, element.x, element.y
        ))
    }

    /// 【内置浏览器】【页面截图】截取视口、整页或单个元素，返回 JPEG 字节。
    /// @param full_page 为是否整页；reference 为可选元素 ref
    /// @returns (JPEG 字节, 截图说明)
    pub(crate) async fn screenshot(
        &self,
        full_page: bool,
        reference: Option<&str>,
    ) -> Result<(Vec<u8>, String)> {
        let mut params = json!({ "format": "jpeg", "quality": 80 });
        // 1. 元素截图按元素边框裁剪；整页截图按内容尺寸裁剪且限制最大高度
        let description = if let Some(reference) = reference {
            let element = self.locate(reference).await?;
            params["clip"] = json!({
                "x": (element.x - element.width / 2.0).max(0.0),
                "y": (element.y - element.height / 2.0).max(0.0),
                "width": element.width,
                "height": element.height,
                "scale": 1,
            });
            format!("element {} [ref={reference}]", element.tag)
        } else if full_page {
            let metrics = self.page_send("Page.getLayoutMetrics", json!({})).await?;
            let size = metrics
                .get("cssContentSize")
                .or_else(|| metrics.get("contentSize"))
                .cloned()
                .unwrap_or(Value::Null);
            let width = size.get("width").and_then(Value::as_f64).unwrap_or(1280.0);
            let height = size
                .get("height")
                .and_then(Value::as_f64)
                .unwrap_or(800.0)
                .min(FULL_PAGE_MAX_HEIGHT);
            params["clip"] =
                json!({ "x": 0, "y": 0, "width": width, "height": height, "scale": 1 });
            params["captureBeyondViewport"] = json!(true);
            format!("full page {width:.0}x{height:.0}")
        } else {
            let (width, height) = self.viewport();
            format!("viewport {width}x{height}")
        };
        // 2. 截图并解码 base64
        let result = self.page_send("Page.captureScreenshot", params).await?;
        let data = result
            .get("data")
            .and_then(Value::as_str)
            .context("screenshot returned no data")?;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data)
            .context("decode screenshot")?;
        Ok((bytes, description))
    }

    /// 【内置浏览器】【面板鼠标】转发面板捕获的鼠标事件。
    /// @param input 为面板鼠标事件
    /// @returns 操作结果
    pub(crate) async fn dispatch_mouse_input(&self, input: &MouseInput) -> Result<()> {
        let button = input.button.as_deref().unwrap_or("none");
        let count = input.click_count.unwrap_or(0);
        match input.kind.as_str() {
            "move" => {
                self.mouse_event("mouseMoved", input.x, input.y, button, 0, input.modifiers)
                    .await
            }
            "down" => {
                self.mouse_event(
                    "mousePressed",
                    input.x,
                    input.y,
                    button,
                    count.max(1),
                    input.modifiers,
                )
                .await
            }
            "up" => {
                self.mouse_event(
                    "mouseReleased",
                    input.x,
                    input.y,
                    button,
                    count.max(1),
                    input.modifiers,
                )
                .await
            }
            "wheel" => self
                .page_send(
                    "Input.dispatchMouseEvent",
                    json!({
                        "type": "mouseWheel",
                        "x": input.x,
                        "y": input.y,
                        "deltaX": input.delta_x,
                        "deltaY": input.delta_y,
                        "modifiers": input.modifiers,
                    }),
                )
                .await
                .map(|_| ()),
            other => bail!("unknown mouse event kind: {other}"),
        }
    }

    /// 【内置浏览器】【鼠标事件】发送一条 Input.dispatchMouseEvent。
    /// @param kind 为事件类型；x、y 为坐标；button 为按键；click_count 为点击次数；modifiers 为修饰键
    /// @returns 操作结果
    async fn mouse_event(
        &self,
        kind: &str,
        x: f64,
        y: f64,
        button: &str,
        click_count: i64,
        modifiers: i64,
    ) -> Result<()> {
        let button = match button {
            "left" | "right" | "middle" | "back" | "forward" => button,
            _ => "none",
        };
        self.page_send(
            "Input.dispatchMouseEvent",
            json!({
                "type": kind,
                "x": x,
                "y": y,
                "button": button,
                "clickCount": click_count,
                "modifiers": modifiers,
            }),
        )
        .await?;
        Ok(())
    }
}
