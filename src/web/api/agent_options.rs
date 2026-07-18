use super::super::app_state::WebAppState;
use crate::config::AppConfig;
use crate::tools;
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

#[derive(Serialize)]
struct ToolOption {
    name: String,
    group: String,
    group_label: String,
    description: String,
}

#[derive(Serialize)]
struct SkillOption {
    name: String,
    description: String,
}

#[derive(Serialize)]
struct AgentOptionsResponse {
    tools: Vec<ToolOption>,
    skills: Vec<SkillOption>,
}

/// 返回 Agent 配置可选项路由。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 挂载在受保护路由组下的路由
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/agent-options", get(list))
        .route("/api/agent-options/mcp", get(list_mcp))
}

/// 枚举 Agent 配置可选的内置工具与 skills。
///
/// 参数:
/// - `state`: Web 应用状态
///
/// 返回:
/// - 工具列表（含分组）与 skill 列表（名称与描述）；任一枚举失败时返回空数组
async fn list(State(state): State<WebAppState>) -> Json<AgentOptionsResponse> {
    // 1. 加载配置，失败时回退到默认配置
    let config = AppConfig::load_or_default(&state.paths).unwrap_or_default();
    // 2. 枚举内置工具及用途分组
    let tool_options = tools::tool_catalog(&config, &state.paths)
        .into_iter()
        .map(tool_option)
        .collect();
    // 3. 扫描 skills 目录，失败时返回空列表
    let skill_options = tools::skill_catalog(&config, &state.paths)
        .unwrap_or_default()
        .into_iter()
        .map(|entry| SkillOption {
            name: entry.name,
            description: entry.description,
        })
        .collect();
    Json(AgentOptionsResponse {
        tools: tool_options,
        skills: skill_options,
    })
}

/// 在后台线程发现 MCP 动态工具。
///
/// 参数:
/// - `state`: Web 应用状态
///
/// 返回:
/// - MCP 动态工具列表；发现线程失败时返回空数组
async fn list_mcp(State(state): State<WebAppState>) -> Json<AgentOptionsResponse> {
    let config = AppConfig::load_or_default(&state.paths).unwrap_or_default();
    if !config.mcp.enabled {
        return Json(AgentOptionsResponse {
            tools: Vec::new(),
            skills: Vec::new(),
        });
    }
    let paths = state.paths.clone();
    let tool_options = tokio::task::spawn_blocking(move || {
        tools::mcp_tool_catalog(&config, &paths)
            .into_iter()
            .map(tool_option)
            .collect()
    })
    .await
    .unwrap_or_default();
    Json(AgentOptionsResponse {
        tools: tool_options,
        skills: Vec::new(),
    })
}

/// 将领域工具目录项转换为接口响应。
fn tool_option(entry: tools::ToolCatalogEntry) -> ToolOption {
    ToolOption {
        name: entry.name,
        group: entry.group.to_string(),
        group_label: entry.group_label.to_string(),
        description: entry.description,
    }
}
