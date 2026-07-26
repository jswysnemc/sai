use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use super::super::services::provider_models;
use crate::config::ProviderConfig;
use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct FetchModelsRequest {
    provider: ProviderConfig,
}

#[derive(Serialize)]
struct FetchModelsResponse {
    models: Vec<String>,
    metadata: std::collections::BTreeMap<String, provider_models::CatalogMetadata>,
}

/// 返回供应商辅助操作路由。
///
/// 返回:
/// - 供应商 API 路由
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/providers/models", post(fetch_models))
        .route("/api/providers/test", post(test_provider))
}

#[derive(Deserialize)]
struct TestProviderRequest {
    provider: ProviderConfig,
    #[serde(default)]
    model: Option<String>,
}

/// 探测供应商连通性。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `request`: 供应商配置与待测模型
///
/// 返回:
/// - 分阶段的探测报告；探测失败不作为 HTTP 错误，而是写进报告
async fn test_provider(
    State(state): State<WebAppState>,
    Json(request): Json<TestProviderRequest>,
) -> WebResult<Json<crate::web::services::provider_probe::ProviderProbeReport>> {
    let provider = provider_models::restore_provider_secret(&state.paths, request.provider)
        .map_err(WebError::from)?;
    let config = crate::config::AppConfig::load_or_default(&state.paths).unwrap_or_default();
    let report = crate::web::services::provider_probe::probe_provider(
        &state.paths,
        &config,
        &provider,
        request.model.as_deref(),
    )
    .await
    .map_err(WebError::from)?;
    Ok(Json(report))
}

/// 使用服务端凭据获取指定供应商模型列表。
///
/// 参数:
/// - `state`: Web 应用状态
/// - `request`: 浏览器提交的供应商配置
///
/// 返回:
/// - 模型标识列表
async fn fetch_models(
    State(state): State<WebAppState>,
    Json(request): Json<FetchModelsRequest>,
) -> WebResult<Json<FetchModelsResponse>> {
    let paths = state.paths.clone();
    let provider = provider_models::restore_provider_secret(&paths, request.provider)
        .map_err(WebError::from)?;
    let result = tokio::task::spawn_blocking(move || {
        let mut result = provider_models::fetch_models(&paths, &provider)?;
        provider_models::enrich_catalog_metadata(&mut result);
        anyhow::Ok(result)
    })
    .await
    .map_err(|error| WebError::from(anyhow::anyhow!(error)))?
    .map_err(|error| WebError::bad_request(error.to_string()))?;
    Ok(Json(FetchModelsResponse {
        models: result.models,
        metadata: result.metadata,
    }))
}
