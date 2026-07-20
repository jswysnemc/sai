use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use crate::config::AppConfig;
use crate::tools;
use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

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

#[derive(Serialize)]
struct ManagedSkillListResponse {
    skills: Vec<tools::ManagedSkill>,
}

#[derive(Serialize)]
struct ManagedSkillDocumentResponse {
    skill: tools::ManagedSkill,
    content: String,
}

#[derive(Deserialize)]
struct CreateSkillRequest {
    directory_name: String,
    content: String,
}

#[derive(Deserialize)]
struct UpdateSkillRequest {
    content: String,
}

#[derive(Deserialize)]
struct SetSkillEnabledRequest {
    enabled: bool,
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
        .route("/api/skills/manage", get(managed_list).post(create))
        .route("/api/skills/manage/:id", get(managed_document).put(update))
        .route("/api/skills/manage/:id/enabled", post(set_enabled))
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

/// 扫描全局与当前人格 Skills，包含已禁用条目。
async fn managed_list(
    State(state): State<WebAppState>,
) -> WebResult<Json<ManagedSkillListResponse>> {
    let config = AppConfig::load_or_default(&state.paths).unwrap_or_default();
    let skills = tools::list_managed_skills(&config, &state.paths).map_err(WebError::from)?;
    Ok(Json(ManagedSkillListResponse { skills }))
}

/// 读取指定 Skill 的原始文档。
async fn managed_document(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
) -> WebResult<Json<ManagedSkillDocumentResponse>> {
    let config = AppConfig::load_or_default(&state.paths).unwrap_or_default();
    let (skill, content) = tools::read_managed_skill(&id, &config, &state.paths)
        .map_err(|error| WebError::not_found(error.to_string()))?;
    Ok(Json(ManagedSkillDocumentResponse { skill, content }))
}

/// 在全局 Skills 目录创建新 Skill。
async fn create(
    State(state): State<WebAppState>,
    Json(request): Json<CreateSkillRequest>,
) -> WebResult<Json<ManagedSkillDocumentResponse>> {
    let id = tools::create_managed_skill(&request.directory_name, &request.content, &state.paths)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    let config = AppConfig::load_or_default(&state.paths).unwrap_or_default();
    let (skill, content) =
        tools::read_managed_skill(&id, &config, &state.paths).map_err(WebError::from)?;
    Ok(Json(ManagedSkillDocumentResponse { skill, content }))
}

/// 更新指定 Skill 的完整文档。
async fn update(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
    Json(request): Json<UpdateSkillRequest>,
) -> WebResult<Json<ManagedSkillDocumentResponse>> {
    let config = AppConfig::load_or_default(&state.paths).unwrap_or_default();
    tools::update_managed_skill(&id, &request.content, &config, &state.paths)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    let (skill, content) =
        tools::read_managed_skill(&id, &config, &state.paths).map_err(WebError::from)?;
    Ok(Json(ManagedSkillDocumentResponse { skill, content }))
}

/// 启用或禁用指定 Skill。
async fn set_enabled(
    State(state): State<WebAppState>,
    Path(id): Path<String>,
    Json(request): Json<SetSkillEnabledRequest>,
) -> WebResult<Json<ManagedSkillDocumentResponse>> {
    let config = AppConfig::load_or_default(&state.paths).unwrap_or_default();
    tools::set_managed_skill_enabled(&id, request.enabled, &config, &state.paths)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    let (skill, content) =
        tools::read_managed_skill(&id, &config, &state.paths).map_err(WebError::from)?;
    Ok(Json(ManagedSkillDocumentResponse { skill, content }))
}
