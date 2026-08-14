use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use crate::config::AppConfig;
use crate::memory::file_store::{Frontmatter, MemoryEntry, MemoryScope, MemoryType};
use crate::memory::MemoryStore;
use axum::extract::{Path, Query, State};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

/// 列表查询参数。
#[derive(Deserialize)]
struct ListQuery {
    limit: Option<usize>,
}

/// 写入请求体。
#[derive(Deserialize)]
struct RememberRequest {
    name: String,
    description: String,
    content: String,
    #[serde(default = "default_type")]
    memory_type: String,
    #[serde(default)]
    global: bool,
}

/// 逐出记录检索参数。
#[derive(Deserialize)]
struct SearchQuery {
    q: String,
    limit: Option<usize>,
}

/// 返回默认条目类型。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 类型标识
fn default_type() -> String {
    "project".to_string()
}

/// 记忆管理路由。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 路由表
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/memory/stats", get(stats))
        .route("/api/memory/entries", get(list).post(remember))
        .route("/api/memory/entries/:name", delete(remove).get(show))
        .route("/api/memory/evicted", get(search_evicted))
        .route("/api/memory/reset", post(reset))
}

/// 打开当前配置对应的记忆入口。
///
/// 参数:
/// - `state`: Web 应用状态
///
/// 返回:
/// - 记忆入口
fn store(state: &WebAppState) -> MemoryStore {
    let config = AppConfig::load_or_default(&state.paths).unwrap_or_default();
    MemoryStore::new(&config, &state.paths)
}

/// 返回记忆状态汇总。
async fn stats(State(state): State<WebAppState>) -> WebResult<Json<Value>> {
    Ok(Json(store(&state).stats().map_err(WebError::from)?))
}

/// 列出记忆条目。
async fn list(
    State(state): State<WebAppState>,
    Query(query): Query<ListQuery>,
) -> WebResult<Json<Value>> {
    Ok(Json(
        store(&state)
            .list_entries(query.limit.unwrap_or(200))
            .map_err(WebError::from)?,
    ))
}

/// 读取一条记忆的完整内容。
async fn show(
    State(state): State<WebAppState>,
    Path(name): Path<String>,
) -> WebResult<Json<Value>> {
    let workspace = crate::runtime_cwd::current_dir().ok();
    let found = store(&state)
        .notes(workspace.as_deref())
        .load(&name)
        .map_err(WebError::from)?;
    let Some((entry, scope)) = found else {
        return Ok(Json(json!({ "found": false, "name": name })));
    };
    Ok(Json(json!({
        "found": true,
        "name": entry.front.name,
        "description": entry.front.description,
        "type": entry.front.memory_type.as_str(),
        "scope": scope_label(scope),
        "content": entry.body,
        "links": entry.links(),
    })))
}

/// 写入或更新一条记忆。
async fn remember(
    State(state): State<WebAppState>,
    Json(request): Json<RememberRequest>,
) -> WebResult<Json<Value>> {
    let memory_type = MemoryType::parse(&request.memory_type)
        .ok_or_else(|| WebError::from(anyhow::anyhow!("未知记忆类型：{}", request.memory_type)))?;
    let scope = if request.global {
        MemoryScope::Global
    } else {
        MemoryScope::Project
    };
    let entry = MemoryEntry {
        front: Frontmatter {
            name: request.name.clone(),
            description: request.description.clone(),
            memory_type,
        },
        body: request.content,
    };
    let workspace = crate::runtime_cwd::current_dir().ok();
    store(&state)
        .notes(workspace.as_deref())
        .save(scope, &entry, &request.description)
        .map_err(WebError::from)?;
    Ok(Json(json!({ "ok": true, "name": request.name })))
}

/// 删除一条记忆。
async fn remove(
    State(state): State<WebAppState>,
    Path(name): Path<String>,
) -> WebResult<Json<Value>> {
    let deleted = store(&state).delete_entry(&name).map_err(WebError::from)?;
    Ok(Json(json!({ "deleted": deleted })))
}

/// 检索被压缩清出上下文的轮次。
async fn search_evicted(
    State(state): State<WebAppState>,
    Query(query): Query<SearchQuery>,
) -> WebResult<Json<Value>> {
    Ok(Json(
        store(&state)
            .search_evicted_context_readonly(&query.q, query.limit.unwrap_or(20))
            .map_err(WebError::from)?,
    ))
}

/// 清空全部记忆。
async fn reset(State(state): State<WebAppState>) -> WebResult<Json<Value>> {
    store(&state).reset_all().map_err(WebError::from)?;
    Ok(Json(json!({ "ok": true })))
}

/// 返回作用域的展示标识。
///
/// 参数:
/// - `scope`: 作用域
///
/// 返回:
/// - 小写标识
fn scope_label(scope: MemoryScope) -> &'static str {
    match scope {
        MemoryScope::Global => "global",
        MemoryScope::Project => "project",
    }
}
