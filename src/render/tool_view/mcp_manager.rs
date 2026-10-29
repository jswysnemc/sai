use super::{result_card, ToolView};
use crate::render::{status_style::ToolHealth, ToolCallDisplayMode};
use serde_json::Value;

/// 【终端】【服务管理】展示连接状态、工具名单或停止结果，隐藏启动命令和协议字段。
/// 参数: 工具视图和模式；返回: MCP 管理结果视图。
pub(super) fn render(view: &ToolView, mode: ToolCallDisplayMode) -> Option<String> {
    if view.name != "mcp_manager" {
        return None;
    }
    let outcome = view.outcome.as_ref().filter(|outcome| outcome.ok)?;
    let value: Value = serde_json::from_str(&outcome.output).ok()?;
    if value["ok"] == false {
        return None;
    }
    let server = result_card::text(&value, "server_id").unwrap_or_default();
    let mut body = Vec::new();
    let title = if let Some(tools) = value["tools"].as_array() {
        body.extend(tools.iter().filter_map(|tool| {
            tool.as_str()
                .map(str::to_string)
                .or_else(|| result_card::text(tool, "name"))
        }));
        format!("MCP {server} · {} tools available", tools.len())
    } else if let Some(servers) = value["servers"].as_array() {
        for server in servers {
            let name = result_card::text(server, "id").unwrap_or_else(|| "Server".into());
            let state = if server["enabled"] == false {
                "disabled"
            } else if server["initialized"] == true {
                "ready"
            } else if server["running"] == true {
                "starting"
            } else {
                "stopped"
            };
            body.push(format!("{name} · {state}"));
            if let Some(error) = result_card::text(server, "last_error") {
                body.push(format!("  {error}"));
            }
        }
        format!("MCP · {} servers", servers.len())
    } else if let Some(stopped) = value["stopped"].as_bool() {
        format!(
            "{} · {server}",
            if stopped {
                "Stopped MCP server"
            } else {
                "MCP server already stopped"
            }
        )
    } else if value["stopped_all"] == true {
        "Stopped all MCP servers".into()
    } else {
        return None;
    };
    Some(result_card::render(
        view,
        mode,
        &title,
        ToolHealth::Ok,
        &body.join("\n"),
    ))
}
