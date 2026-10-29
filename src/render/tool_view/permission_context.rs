//! 审批前的操作对象与范围说明，不展示任意参数或敏感正文。

use super::ToolView;
use crate::render::terminal_text as t;
use serde_json::Value;

/// 【终端】【审批上下文】从已解析参数提取操作与目标，放在审批选项之前。
/// 参数: `view` 为工具生命周期；返回: 待审批说明，已决策或无审批时返回空。
pub(crate) fn render(view: &ToolView) -> String {
    if !view
        .permission
        .as_ref()
        .is_some_and(|audit| audit.decision.is_none())
    {
        return String::new();
    }
    let args = serde_json::from_str::<Value>(&view.arguments).unwrap_or_default();
    let action = args["action"].as_str().unwrap_or(&view.name);
    let mut rows = vec![format!("{}: {}", t("Action", "操作"), clean(action))];
    // 1. 【终端】【审批对象】只使用目标字段，不读取正文、口令或任意嵌套字段
    for (key, label) in [
        ("command", t("Command", "命令")),
        ("path", t("Path", "路径")),
        ("url", "URL"),
        ("app", t("Application", "应用")),
        ("target", t("Target", "目标")),
        ("window_id", t("Window", "窗口")),
        ("cwd", t("Working directory", "工作目录")),
        ("scope", t("Requested scope", "请求范围")),
    ] {
        if let Some(value) = args
            .get(key)
            .filter(|value| value.is_string() || value.is_number())
        {
            let text = value
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| value.to_string());
            if !text.trim().is_empty() {
                rows.push(format!("{label}: {}", clean(&text)));
            }
        }
    }
    if view
        .permission
        .as_ref()
        .is_some_and(|audit| audit.sandbox.is_none())
    {
        rows.push(t("Review scope: this tool call", "当前审阅对象为本次工具调用").into());
    }
    format!("\n  {}", rows.join("\n  "))
}

/// 【终端】【审批文本】移除终端控制符，保留参数换行但对齐续行。
/// 参数: `text` 为参数；返回: 安全可读文本。
fn clean(text: &str) -> String {
    super::safe_text::plain(text)
        .replace('\n', "\n    ")
        .replace('\t', "    ")
}
