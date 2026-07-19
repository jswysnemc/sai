use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use super::super::workspace;
use axum::extract::{Query, State};
use axum::http::header::CONTENT_TYPE;
use axum::http::HeaderValue;
use axum::response::Response;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

#[derive(Deserialize)]
struct TreeQuery {
    path: Option<String>,
    depth: Option<usize>,
}

#[derive(Deserialize)]
struct FileQuery {
    path: String,
}

#[derive(Deserialize)]
struct SaveFileRequest {
    path: String,
    content: String,
    expected_modified_at: Option<u64>,
}

#[derive(Deserialize)]
struct CreateEntryRequest {
    path: String,
    kind: String,
}

#[derive(Deserialize)]
struct RenameEntryRequest {
    from: String,
    to: String,
}

#[derive(Deserialize)]
struct DeleteEntryRequest {
    path: String,
}

/// 返回工作区文件与 Diff 路由。
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/workspace/tree", get(tree))
        .route("/api/workspace/file", get(file).put(save_file))
        .route("/api/workspace/image", get(image))
        .route(
            "/api/workspace/entry",
            axum::routing::post(create_entry)
                .patch(rename_entry)
                .delete(delete_entry),
        )
        .route("/api/workspace/diff", get(diff))
}

/// 返回用于编辑器预览的图像文件。
async fn image(
    State(state): State<WebAppState>,
    Query(query): Query<FileQuery>,
) -> WebResult<Response> {
    let active = state.workspaces.active().map_err(WebError::from)?;
    let image = workspace::read_image(std::path::Path::new(&active.path), &query.path)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    let content_type = HeaderValue::from_str(&image.mime)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Response::builder()
        .header(CONTENT_TYPE, content_type)
        .body(axum::body::Body::from(image.bytes))
        .map_err(anyhow::Error::from)
        .map_err(WebError::from)
}

/// 读取文件树。
async fn tree(
    State(state): State<WebAppState>,
    Query(query): Query<TreeQuery>,
) -> WebResult<Json<Vec<workspace::FileNode>>> {
    let active = state.workspaces.active().map_err(WebError::from)?;
    let nodes = workspace::read_tree(
        std::path::Path::new(&active.path),
        query.path.as_deref().unwrap_or(""),
        query.depth.unwrap_or(4),
    )
    .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(nodes))
}

/// 读取文本文件。
async fn file(
    State(state): State<WebAppState>,
    Query(query): Query<FileQuery>,
) -> WebResult<Json<workspace::FileContent>> {
    let active = state.workspaces.active().map_err(WebError::from)?;
    let file = workspace::read_file(std::path::Path::new(&active.path), &query.path)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(file))
}

/// 保存文本文件。
async fn save_file(
    State(state): State<WebAppState>,
    Json(request): Json<SaveFileRequest>,
) -> WebResult<Json<workspace::FileContent>> {
    let active = state.workspaces.active().map_err(WebError::from)?;
    if let Some(expected) = request.expected_modified_at {
        let current = workspace::read_file(std::path::Path::new(&active.path), &request.path)
            .map_err(|error| WebError::bad_request(error.to_string()))?;
        if current.modified_at != Some(expected) {
            return Err(WebError::conflict(
                "file changed outside the editor; review the latest content before saving",
            ));
        }
    }
    let file = workspace::write_file(
        std::path::Path::new(&active.path),
        &request.path,
        &request.content,
    )
    .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(file))
}

/// 创建工作区文件或目录。
async fn create_entry(
    State(state): State<WebAppState>,
    Json(request): Json<CreateEntryRequest>,
) -> WebResult<Json<workspace::FileMutation>> {
    let active = state.workspaces.active().map_err(WebError::from)?;
    let entry = workspace::create_entry(
        std::path::Path::new(&active.path),
        &request.path,
        request.kind == "directory",
    )
    .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(entry))
}

/// 重命名工作区文件或目录。
async fn rename_entry(
    State(state): State<WebAppState>,
    Json(request): Json<RenameEntryRequest>,
) -> WebResult<Json<workspace::FileMutation>> {
    let active = state.workspaces.active().map_err(WebError::from)?;
    let entry = workspace::rename_entry(
        std::path::Path::new(&active.path),
        &request.from,
        &request.to,
    )
    .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(entry))
}

/// 删除工作区文件或目录。
async fn delete_entry(
    State(state): State<WebAppState>,
    Json(request): Json<DeleteEntryRequest>,
) -> WebResult<Json<workspace::FileMutation>> {
    let active = state.workspaces.active().map_err(WebError::from)?;
    let entry = workspace::delete_entry(std::path::Path::new(&active.path), &request.path)
        .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(entry))
}

/// 读取当前工作区 Git Diff。
async fn diff(State(state): State<WebAppState>) -> WebResult<Json<workspace::GitDiff>> {
    let active = state.workspaces.active().map_err(WebError::from)?;
    let diff = workspace::read_git_diff(std::path::Path::new(&active.path))
        .await
        .map_err(WebError::from)?;
    Ok(Json(diff))
}
