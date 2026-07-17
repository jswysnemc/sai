use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use crate::config::AppConfig;
use crate::tools;
use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

#[derive(Serialize)]
struct SkillListItem {
    name: String,
    description: String,
}

#[derive(Serialize)]
struct SkillListResponse {
    skills: Vec<SkillListItem>,
}

#[derive(Serialize)]
struct SkillDocumentResponse {
    name: String,
    description: String,
    content: String,
}

/// 返回 skills 目录与文档路由。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 挂载在受保护路由组下的路由
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/skills", get(list))
        .route("/api/skills/:name", get(document))
}

/// 枚举当前可用 skills 的名称与描述。
///
/// 参数:
/// - `state`: Web 应用状态
///
/// 返回:
/// - skill 列表
async fn list(State(state): State<WebAppState>) -> WebResult<Json<SkillListResponse>> {
    let config = AppConfig::load_or_default(&state.paths).unwrap_or_default();
    let skills = tools::skill_catalog(&config, &state.paths)
        .map_err(WebError::from)?
        .into_iter()
        .map(|entry| SkillListItem {
            name: entry.name,
            description: entry.description,
        })
        .collect();
    Ok(Json(SkillListResponse { skills }))
}

/// 读取指定 skill 的完整文档，供消息发送前注入模型上下文。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `name`: skill 名称
///
/// 返回:
/// - skill 名称、描述与完整 SKILL.md 包装文本
async fn document(
    State(state): State<WebAppState>,
    Path(name): Path<String>,
) -> WebResult<Json<SkillDocumentResponse>> {
    let config = AppConfig::load_or_default(&state.paths).unwrap_or_default();
    let content =
        tools::load_installed_skill_document(&name, &config, &state.paths).map_err(|error| {
            let message = error.to_string();
            if message.starts_with("skill not found:") {
                WebError::not_found(message)
            } else {
                WebError::from(error)
            }
        })?;
    let description = tools::skill_catalog(&config, &state.paths)
        .ok()
        .and_then(|entries| {
            entries
                .into_iter()
                .find(|entry| entry.name == name)
                .map(|entry| entry.description)
        })
        .unwrap_or_default();
    Ok(Json(SkillDocumentResponse {
        name,
        description,
        content,
    }))
}
