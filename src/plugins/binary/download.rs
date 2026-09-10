use super::public_address;
use anyhow::{bail, Context, Result};
use sai_plugin_runtime::host::{BinaryResponse, HttpRequest};
use sai_plugin_runtime::Capabilities;
use std::time::{Duration, Instant};

/// 【插件下载】【匿名入口】只执行无正文、无自定义头的 GET，并对整条下载链设置截止时间。
/// @param request 下载请求；capabilities 为精确来源及公开网络授权
/// @returns 有界响应，不将服务器返回的 Cookie 发送到后续地址
pub(in crate::plugins) async fn download(
    request: HttpRequest,
    capabilities: Capabilities,
) -> Result<BinaryResponse> {
    super::validate_limit(request.max_bytes)?;
    if request.method != "GET" || request.body.is_some() || !request.headers.is_empty() {
        bail!("binary download requires an anonymous GET without headers or body");
    }
    let timeout = Duration::from_millis(request.timeout_ms.clamp(1, 600_000));
    tokio::time::timeout(timeout, follow(request, capabilities, timeout))
        .await
        .context("plugin binary download timed out")?
}

/// 【插件下载】【逐跳授权】每次跳转重新判断来源并解析地址，公开权限不能开放本地服务。
/// @param request 无凭据请求；capabilities 为授权；timeout 为整条链时限
/// @returns 最终响应及正文
async fn follow(
    request: HttpRequest,
    capabilities: Capabilities,
    timeout: Duration,
) -> Result<BinaryResponse> {
    let deadline = Instant::now() + timeout;
    let mut url = public_address::parse(&request.url)?;
    for redirects in 0..=5 {
        let builder = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none());
        let builder = if capabilities
            .http
            .contains(&url.origin().ascii_serialization())
        {
            builder
        } else if capabilities.binary.public_downloads {
            public_address::pin(builder, &url).await?
        } else {
            bail!("plugin download origin is not allowed");
        };
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|duration| !duration.is_zero())
            .context("plugin binary download timed out")?;
        let response = builder
            .timeout(remaining)
            .build()?
            .get(url.clone())
            .send()
            .await
            .map_err(|error| error.without_url())
            .context("plugin binary download failed")?;
        if !matches!(response.status().as_u16(), 301 | 302 | 303 | 307 | 308) {
            return super::read(response, request.max_bytes).await;
        }
        let Some(location) = response.headers().get(reqwest::header::LOCATION) else {
            return super::read(response, request.max_bytes).await;
        };
        if redirects == 5 {
            bail!("plugin download redirect limit exceeded");
        }
        let target = url
            .join(
                location
                    .to_str()
                    .context("invalid plugin download redirect")?,
            )
            .context("invalid plugin download redirect target")?;
        url = public_address::parse(target.as_str())?;
    }
    unreachable!("download redirect loop always returns a response or error")
}
