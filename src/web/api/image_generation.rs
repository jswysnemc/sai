use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use super::super::services::config_service::SECRET_SENTINEL;
use crate::config::{ModelEndpointConfig, ModelEndpointKind};
use crate::tools::image_generation::request::{model_catalog_url, ImageAuth};
use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Instant;

#[derive(Debug, Deserialize)]
struct EndpointRequest {
    endpoint: ModelEndpointConfig,
}

#[derive(Debug, Deserialize)]
struct GenerateRequest {
    endpoint_id: String,
    #[serde(default)]
    model: Option<String>,
    prompt: String,
    #[serde(default)]
    aspect_ratio: String,
    #[serde(default)]
    resolution: String,
}

#[derive(Debug, Serialize)]
struct ModelsResponse {
    models: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ProbeStage {
    stage: &'static str,
    ok: bool,
    duration_ms: u64,
    detail: String,
}

#[derive(Debug, Serialize)]
struct ProbeResponse {
    ok: bool,
    endpoint_id: String,
    model: String,
    total_ms: u64,
    stages: Vec<ProbeStage>,
}

/// 返回生图模型的目录、探测和直接生成接口。
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/image-models/models", post(fetch_models))
        .route("/api/image-models/test", post(test_endpoint))
        .route("/api/image-models/generate", post(generate_image))
}

/// 获取生图端点公开的模型目录。
async fn fetch_models(
    State(state): State<WebAppState>,
    Json(request): Json<EndpointRequest>,
) -> WebResult<Json<ModelsResponse>> {
    let endpoint = restore_endpoint(&state, request.endpoint)?;
    let models = request_models(&endpoint).await.map_err(WebError::from)?;
    Ok(Json(ModelsResponse { models }))
}

/// 用一次最小图片请求验证生图端点、密钥和模型是否可用。
async fn test_endpoint(
    State(state): State<WebAppState>,
    Json(request): Json<EndpointRequest>,
) -> WebResult<Json<ProbeResponse>> {
    let endpoint = restore_endpoint(&state, request.endpoint)?;
    let started = Instant::now();
    let result = crate::tools::image_generation::generate(
        json!({
            "endpoint_id": endpoint.id,
            "prompt": "A simple geometric test image",
            "resolution": "256x256"
        }),
        vec![endpoint.clone()],
        state.paths.cache_dir.join("generated-images"),
        crate::tools::ToolProgress::new(tokio::sync::mpsc::unbounded_channel().0),
    )
    .await;
    let duration_ms = started.elapsed().as_millis() as u64;
    let stage = match result {
        Ok(_) => ProbeStage {
            stage: "image_generation",
            ok: true,
            duration_ms,
            detail: "Image endpoint returned a valid image".to_string(),
        },
        Err(error) => ProbeStage {
            stage: "image_generation",
            ok: false,
            duration_ms,
            detail: error.to_string(),
        },
    };
    Ok(Json(ProbeResponse {
        ok: stage.ok,
        endpoint_id: endpoint.id,
        model: endpoint.model,
        total_ms: duration_ms,
        stages: vec![stage],
    }))
}

/// 根据聊天输入调用选定的生图端点，并返回可直接渲染的图片摘要。
async fn generate_image(
    State(state): State<WebAppState>,
    Json(request): Json<GenerateRequest>,
) -> WebResult<Json<Value>> {
    if request.prompt.trim().is_empty() {
        return Err(WebError::bad_request("prompt cannot be empty"));
    }
    let config = crate::config::AppConfig::load_or_default(&state.paths).map_err(WebError::from)?;
    let mut endpoint = config
        .model_endpoints
        .iter()
        .find(|item| {
            item.id == request.endpoint_id && item.kind == ModelEndpointKind::ImageGeneration
        })
        .cloned()
        .ok_or_else(|| WebError::not_found("image model endpoint not found"))?;
    if let Some(model) = request
        .model
        .as_deref()
        .map(str::trim)
        .filter(|model| !model.is_empty())
    {
        endpoint.model = model.to_string();
    }
    let output = crate::tools::image_generation::generate(
        json!({
            "endpoint_id": endpoint.id,
            "prompt": request.prompt,
            "aspect_ratio": request.aspect_ratio,
            "resolution": request.resolution
        }),
        vec![endpoint],
        state.paths.cache_dir.join("generated-images"),
        crate::tools::ToolProgress::new(tokio::sync::mpsc::unbounded_channel().0),
    )
    .await
    .map_err(|error| WebError::bad_request(error.to_string()))?;
    let value =
        serde_json::from_str(&output).map_err(|error| WebError::from(anyhow::anyhow!(error)))?;
    Ok(Json(value))
}

/// 将脱敏的浏览器草稿与服务端保存的真实密钥合并。
fn restore_endpoint(
    state: &WebAppState,
    mut submitted: ModelEndpointConfig,
) -> WebResult<ModelEndpointConfig> {
    if submitted.kind != ModelEndpointKind::ImageGeneration {
        return Err(WebError::bad_request(
            "endpoint is not an image generation endpoint",
        ));
    }
    let config = crate::config::AppConfig::load_or_default(&state.paths).map_err(WebError::from)?;
    let current = config
        .model_endpoints
        .iter()
        .find(|item| item.id == submitted.id)
        .cloned();
    if submitted.api_key == SECRET_SENTINEL {
        submitted.api_key = current
            .as_ref()
            .map(|item| item.api_key.clone())
            .unwrap_or_default();
    }
    if let Some(current) = current {
        for key in &mut submitted.api_keys {
            if key.api_key == SECRET_SENTINEL {
                if let Some(previous) = current.api_keys.iter().find(|item| item.id == key.id) {
                    key.api_key = previous.api_key.clone();
                }
            }
        }
    }
    Ok(submitted)
}

/// 请求 OpenAI 兼容接口的模型目录，兼容带版本路径和完整生图路径的地址。
async fn request_models(endpoint: &ModelEndpointConfig) -> anyhow::Result<Vec<String>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()?;
    let key = endpoint.resolved_api_key()?;
    let (url, auth) = model_catalog_url(endpoint);
    let mut last_error = String::from("model endpoint returned no result");
    for candidate in [url] {
        let mut request = client.get(&candidate).header("Accept", "application/json");
        if !key.is_empty() {
            request = match auth {
                ImageAuth::Bearer => request.bearer_auth(&key),
                ImageAuth::GeminiQueryKey => request.query(&[("key", key.as_str())]),
            };
        }
        let response = request.send().await?;
        let status = response.status();
        let body = response.text().await?;
        if status.is_success() {
            return Ok(parse_models(&body));
        }
        last_error = format!("{status}: {body}");
        if status.as_u16() != 404 {
            break;
        }
    }
    anyhow::bail!(last_error)
}

/// 从常见模型目录响应中提取模型标识。
fn parse_models(body: &str) -> Vec<String> {
    let value = serde_json::from_str::<Value>(body).ok();
    let values = value
        .as_ref()
        .and_then(|value| value.get("data").or_else(|| value.get("models")))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut models = values
        .iter()
        .filter_map(|item| {
            item.as_str()
                .or_else(|| item.get("id").and_then(Value::as_str))
        })
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    models.sort();
    models.dedup();
    models
}
