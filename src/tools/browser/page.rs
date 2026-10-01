//! 页面级浏览器工具：导航、标签页管理与条件等待。

use super::{number_arg, optional_str, required_str, session_for};
use crate::browser::navigation::HistoryStep;
use crate::browser::normalize_url;
use anyhow::{bail, Result};
use serde_json::Value;
use std::time::Duration;

/// 【浏览器工具】【页面导航】执行打开地址或历史导航。
/// @param args 为工具参数
/// @returns 导航后的页面概况
pub(super) async fn navigate(args: Value) -> Result<String> {
    let action = optional_str(&args, "action").unwrap_or("navigate");
    let summary = match action {
        "navigate" => {
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
        "{}\nCall browser with action=snapshot to read the page.",
        summary.describe()
    ))
}

/// 【浏览器工具】【标签管理】执行标签页的列举、新建、切换与关闭。
/// @param args 为工具参数
/// @returns 操作后的标签列表
pub(super) async fn tabs(args: Value) -> Result<String> {
    let action = required_str(&args, "action")?;
    let session = session_for(&format!("Tabs: {action}")).await?;
    // 1. 按动作执行，new 的地址同样经过地址策略
    match action {
        "tabs" => {}
        "new_tab" => {
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
        "switch_tab" => session.switch_tab(required_str(&args, "id")?).await?,
        "close_tab" => session.close_tab(optional_str(&args, "id")).await?,
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
pub(super) async fn wait(args: Value) -> Result<String> {
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
