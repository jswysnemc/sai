mod download;
mod files;
mod paths;
mod public_address;
mod terminal;

pub(super) use download::download;
pub(super) use files::write;
pub(super) use terminal::display;

use anyhow::{bail, Result};
use sai_plugin_runtime::host::{BinaryResponse, HttpRequest};
use sai_plugin_runtime::Capabilities;

/// 【插件二进制】【精确请求】复用 HTTP 来源及重定向授权，保留响应原始字节。
/// @param request 请求；capabilities 为来源授权；allow_writes 为可信写入权限
/// @returns 受独立字节及时间上限约束的正文
pub(super) async fn request(
    request: HttpRequest,
    capabilities: Capabilities,
    allow_writes: bool,
) -> Result<BinaryResponse> {
    validate_limit(request.max_bytes)?;
    let max_bytes = request.max_bytes;
    let response =
        super::http::send_with_timeout_limit(request, capabilities, allow_writes, 600_000).await?;
    read(response, max_bytes).await
}

/// 【插件二进制】【正文读取】状态和响应头属于元数据，正文始终保留为字节数组。
/// @param response 最终响应；max_bytes 为本次上限
/// @returns 不进行文本解码的结果
async fn read(response: reqwest::Response, max_bytes: usize) -> Result<BinaryResponse> {
    let status = response.status().as_u16();
    let url = response.url().to_string();
    let headers = response
        .headers()
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.to_string(), value.to_string()))
        })
        .collect();
    let body = super::http::read_bytes(response, max_bytes).await?;
    Ok(BinaryResponse {
        status,
        url,
        headers,
        body,
    })
}

/// 【插件二进制】【硬上限】即使直接调用宿主也不能绕过最大响应限制。
/// @param max_bytes 本次字节上限
/// @returns 非零且不超过 64 MiB 时成功
fn validate_limit(max_bytes: usize) -> Result<()> {
    if max_bytes == 0 || max_bytes > 64 * 1024 * 1024 {
        bail!("plugin binary byte limit is outside supported bounds");
    }
    Ok(())
}
