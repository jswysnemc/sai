use super::{result_card, ToolView};
use crate::render::{status_style::ToolHealth, terminal_text as t, ToolCallDisplayMode};

/// 【终端】【交互命令】展示输入会话与新输出，运行中的命令不显示为已完成。
/// 参数：工具视图与展示模式；返回交互会话卡片，普通工具为空。
pub(super) fn render(view: &ToolView, mode: ToolCallDisplayMode) -> Option<String> {
    let output = &view.outcome.as_ref()?.output;
    let value: serde_json::Value = serde_json::from_str(output).ok()?;
    if value["mode"] != "interactive" {
        return None;
    }
    let running = value["running"] == true;
    let title = if running {
        t("Interactive command running", "交互命令运行中")
    } else if value["success"] == true {
        t("Interactive command completed", "交互命令已完成")
    } else {
        t("Interactive command stopped", "交互命令已停止")
    };
    let mut body = String::new();
    if let Some(id) = value["session_id"].as_str() {
        body.push_str(&format!("{}: {id}\n", t("Session", "会话")));
    }
    if let Some(output) = value["stdout"].as_str() {
        body.push_str(&crate::render::ansi_clip::strip_ansi(output).replace('\r', ""));
    }
    if let Some(omitted) = value["omitted_bytes"].as_u64().filter(|count| *count > 0) {
        body.push_str(&format!(
            "\n{}: {omitted} bytes",
            t("Earlier output omitted", "已省略较早输出")
        ));
    }
    Some(result_card::render(
        view,
        mode,
        title,
        if running {
            ToolHealth::Pending
        } else if value["success"] == true {
            ToolHealth::Ok
        } else {
            ToolHealth::Err
        },
        &body,
    ))
}
