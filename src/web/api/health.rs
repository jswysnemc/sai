use axum::Json;
use serde::Serialize;

#[derive(Serialize)]
pub(super) struct HealthResponse {
    ok: bool,
    version: &'static str,
}

/// 返回服务健康状态。
pub(super) async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        ok: true,
        version: env!("CARGO_PKG_VERSION"),
    })
}
