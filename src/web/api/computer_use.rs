use super::super::app_state::WebAppState;
use super::super::error::{WebError, WebResult};
use super::super::services::computer_use::{self, DriverStatus, InstallResult};
use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};

/// 【桌面操作】【路由】返回受认证保护的驱动状态与安装接口。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 状态查询与安装路由
pub(super) fn routes() -> Router<WebAppState> {
    Router::new()
        .route("/api/computer-use/status", get(status))
        .route("/api/computer-use/install", post(install))
}

/// 【桌面操作】【状态查询】读取配置中的驱动路径并探测本机二进制，不启动 MCP。
///
/// 参数:
/// - `state`: Web 应用状态
///
/// 返回:
/// - 驱动探测结果
async fn status(State(state): State<WebAppState>) -> WebResult<Json<DriverStatus>> {
    let config = crate::config::AppConfig::load_or_default(&state.paths).map_err(WebError::from)?;
    Ok(Json(
        computer_use::status(&state.paths, &config.computer_use.driver_path).await,
    ))
}

/// 【桌面操作】【驱动安装】下载官方发布包到 Sai 数据目录，不开启功能也不授予系统权限。
///
/// 参数:
/// - `state`: Web 应用状态
///
/// 返回:
/// - 安装后的二进制路径与版本
async fn install(State(state): State<WebAppState>) -> WebResult<Json<InstallResult>> {
    let paths = state.paths.clone();
    let result = tokio::task::spawn_blocking(move || computer_use::install(&paths))
        .await
        .map_err(|error| WebError::from(anyhow::anyhow!(error)))?
        .map_err(WebError::from)?;
    Ok(Json(result))
}
