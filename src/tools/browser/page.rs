//! 页面级浏览器工具：导航、标签页管理与条件等待。

use super::{number_arg, optional_str, required_str, session_for};
use crate::browser::navigation::HistoryStep;
use crate::browser::normalize_url;
use crate::tools::{ToolRegistry, ToolSpec};
use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::time::Duration;

/// 【浏览器工具】【页面工具注册】注册导航、标签页与等待工具。
/// @param registry 为工具注册表
/// @returns 无
pub(super) fn register(registry: &mut ToolRegistry) {
    registry.register(ToolSpec::new(
        "browser_navigate",
        "Open a URL in the built-in browser, or go back/forward/reload. The user watches the same page live in the Web workbench browser panel. Only http(s) URLs are allowed. After navigating, call browser_snapshot to read the page and get element refs.",
        json!({
            "type": "object",
            "properties": {
                "url": {"type": "string", "description": "URL to open. A bare host such as example.com gets https:// (localhost gets http://). Required when action is goto."},
                "action": {"type": "string", "enum": ["goto", "back", "forward", "reload"], "description": "Navigation action; defaults to goto."}
            },
            "additionalProperties": false
        }),
        |args| async move { navigate(args).await },
    ));
    registry.register(ToolSpec::new(
        "browser_tabs",
        "List, open, switch or close tabs in the built-in browser. Tab ids are short ids from the list action. Links that open a new window switch to the new tab automatically.",
        json!({
            "type": "object",
            "properties": {
                "action": {"type": "string", "enum": ["list", "new", "switch", "close"], "description": "Tab action."},
                "id": {"type": "string", "description": "Tab id for switch or close; close defaults to the current tab."},
                "url": {"type": "string", "description": "Initial URL for new; defaults to about:blank."}
            },
            "required": ["action"],
            "additionalProperties": false
        }),
        |args| async move { tabs(args).await },
    ));
    registry.register(ToolSpec::new(
        "browser_wait",
        "Wait until the current page shows some text or a CSS selector matches, or just wait a number of seconds for dynamic content.",
        json!({
            "type": "object",
            "properties": {
                "text": {"type": "string", "description": "Wait until the page text contains this string."},
                "selector": {"type": "string", "description": "Wait until document.querySelector(selector) matches."},
                "seconds": {"type": "number", "description": "Timeout in seconds (1-60, default 10); with no text or selector, the time to wait."}
            },
            "additionalProperties": false
        }),
        |args| async move { wait(args).await },
    ));
}

/// 【浏览器工具】【页面导航】执行打开地址或历史导航。
/// @param args 为工具参数
/// @returns 导航后的页面概况
async fn navigate(args: Value) -> Result<String> {
    let action = optional_str(&args, "action").unwrap_or("goto");
    let summary = match action {
        "goto" => {
            // 1. 模型给出的地址不做搜索转换，只补全协议并校验
            let url = normalize_url(required_str(&args, "url")?, false)?;
            let session = session_for(&format!("Opening {url}")).await?;
            session.navigate(&url).await?
        }
        "back" => {
            session_for("Going back")
                .await?
                .go_history(HistoryStep::Back)
                .await?
        }
        "forward" => {
            session_for("Going forward")
                .await?
                .go_history(HistoryStep::Forward)
                .await?
        }
        "reload" => session_for("Reloading page").await?.reload().await?,
        other => bail!("unknown navigate action: {other}"),
    };
    Ok(format!(
        "{}\nCall browser_snapshot to read the page.",
        summary.describe()
    ))
}

/// 【浏览器工具】【标签管理】执行标签页的列举、新建、切换与关闭。
/// @param args 为工具参数
/// @returns 操作后的标签列表
async fn tabs(args: Value) -> Result<String> {
    let action = required_str(&args, "action")?;
    let session = session_for(&format!("Tabs: {action}")).await?;
    // 1. 按动作执行，new 的地址同样经过地址策略
    match action {
        "list" => {}
        "new" => {
            let url = match optional_str(&args, "url") {
                Some(url) => normalize_url(url, false)?,
                None => "about:blank".to_string(),
            };
            session.new_tab(&url).await?;
            if url != "about:blank" {
                session
                    .wait_until_ready(crate::browser::navigation::LOAD_TIMEOUT)
                    .await?;
            }
        }
        "switch" => session.switch_tab(required_str(&args, "id")?).await?,
        "close" => session.close_tab(optional_str(&args, "id")).await?,
        other => bail!("unknown tabs action: {other}"),
    }
    // 2. 统一返回最新标签列表
    let lines: Vec<String> = session
        .list_tabs()
        .await?
        .into_iter()
        .map(|tab| {
            format!(
                "{} [{}] {} — {}",
                if tab.active { "*" } else { " " },
                tab.id,
                if tab.title.is_empty() {
                    "(untitled)"
                } else {
                    &tab.title
                },
                tab.url
            )
        })
        .collect();
    Ok(format!("Tabs (* = current):\n{}", lines.join("\n")))
}

/// 【浏览器工具】【条件等待】等待文本、选择器或固定时长。
/// @param args 为工具参数
/// @returns 满足条件后的页面概况
async fn wait(args: Value) -> Result<String> {
    let seconds = number_arg(&args, "seconds", 10.0, 1.0, 60.0);
    let text = optional_str(&args, "text");
    let selector = optional_str(&args, "selector");
    let session = session_for("Waiting for page").await?;
    let summary = session
        .wait_for(text, selector, Duration::from_secs_f64(seconds))
        .await?;
    let condition = match (text, selector) {
        (Some(text), _) => format!("Text {text:?} appeared."),
        (None, Some(selector)) => format!("Selector {selector:?} matched."),
        (None, None) => format!("Waited {seconds} s."),
    };
    Ok(format!("{condition}\n{}", summary.describe()))
}
