mod response;
mod storage;

use super::{ToolProgress, ToolRegistry, ToolSpec};
use crate::config::{AppConfig, ModelEndpointConfig, ModelEndpointKind};
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::{bail, Context, Result};
use response::{client, decode_response};
use serde_json::{json, Value};
use std::time::Duration;

const MAX_PROMPT_CHARS: usize = 20_000;

/// 在已有的专用图片端点存在时注册统一的生图工具。
///
/// 参数:
/// - registry: 当前会话工具表
/// - config: 应用配置，包含专用图片端点
/// - paths: Sai 路径集合
///
/// 返回:
/// - 无；没有配置图片端点时不注册工具
pub(super) fn register(registry: &mut ToolRegistry, config: &AppConfig, paths: &SaiPaths) {
    if !config
        .model_endpoints
        .iter()
        .any(|endpoint| endpoint.kind == ModelEndpointKind::ImageGeneration)
    {
        return;
    }
    let endpoints = config.model_endpoints.clone();
    let cache_dir = paths.cache_dir.join("generated-images");
    registry.register(ToolSpec::new_with_progress(
        "generate_image",
        t(
            "Generate one or more images with the configured image model.",
            "使用已配置的生图模型生成图片。",
        ),
        json!({
            "type": "object",
            "properties": {
                "prompt": {
                    "type": "string",
                    "description": t("Image prompt.", "图片提示词。"),
                    "maxLength": MAX_PROMPT_CHARS
                },
                "aspect_ratio": {
                    "type": "string",
                    "description": t("Optional aspect ratio, such as 16:9.", "可选画面比例，例如 16:9。")
                },
                "resolution": {
                    "type": "string",
                    "description": t("Optional resolution, such as 1024x1024 or 2K.", "可选分辨率，例如 1024x1024 或 2K。")
                },
                "endpoint_id": {
                    "type": "string",
                    "description": t("Optional configured image endpoint ID.", "可选的图片端点 ID。")
                }
            },
            "required": ["prompt"],
            "additionalProperties": false
        }),
        move |args, progress| {
            let endpoints = endpoints.clone();
            let cache_dir = cache_dir.clone();
            async move { generate(args, endpoints, cache_dir, progress).await }
        },
    ));
}

/// 向选定专用端点发送请求并把所有图片缓存为本地媒体文件。
///
/// 参数:
/// - args: prompt、aspect_ratio、resolution 和可选 endpoint_id
/// - endpoints: 应用配置中的专用模型端点
/// - cache_dir: 生成图片缓存目录
/// - progress: 工具进度通道
///
/// 返回:
/// - 小体积 JSON 摘要，包含图片本地路径和 Web 媒体 URL
async fn generate(
    args: Value,
    endpoints: Vec<ModelEndpointConfig>,
    cache_dir: std::path::PathBuf,
    progress: ToolProgress,
) -> Result<String> {
    let prompt = args
        .get("prompt")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context("prompt is required")?;
    if prompt.chars().count() > MAX_PROMPT_CHARS {
        bail!("prompt exceeds {} characters", MAX_PROMPT_CHARS);
    }
    let endpoint = select_endpoint(&endpoints, args.get("endpoint_id").and_then(Value::as_str))?;
    let model = endpoint.model.trim();
    let mut request = serde_json::Map::new();
    if !model.is_empty() {
        request.insert("model".into(), Value::String(model.to_string()));
    }
    request.insert("prompt".into(), Value::String(prompt.to_string()));
    request.insert("n".into(), Value::Number(1.into()));
    add_string_field(&mut request, &args, "aspect_ratio");
    add_string_field(&mut request, &args, "resolution");
    add_string_field_as_size(&mut request, &args, "resolution");

    progress.report(format!("请求图片模型：{}", endpoint.name));
    let client = client()?;
    let api_key = endpoint.resolved_api_key()?;
    let mut request_builder = client
        .post(&endpoint.endpoint)
        .timeout(Duration::from_secs(180))
        .json(&Value::Object(request));
    if !api_key.trim().is_empty() {
        request_builder = request_builder.bearer_auth(api_key.trim());
    }
    progress.report("图片生成中".to_string());
    let response = request_builder.send().await?.error_for_status()?;
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let body = response.bytes().await?.to_vec();
    let images = decode_response(body, content_type.as_deref(), client).await?;
    progress.report(format!("已收到 {} 张图片，正在保存", images.len()));
    let mut stored = Vec::new();
    for (index, image) in images.iter().enumerate() {
        let image = storage::store(&cache_dir, image, index)?;
        progress.report(format!("正在处理图片 {}/{}", index + 1, images.len()));
        stored.push(json!({
            "local_path": image.path,
            "url": format!("/api/generated-images/{}", image.file_name),
            "mime": image.mime,
            "bytes": image.bytes,
            "source": images[index].source,
        }));
    }
    Ok(serde_json::to_string(&json!({
        "type": "image_generation",
        "status": "completed",
        "endpoint": endpoint.name,
        "model": endpoint.model,
        "prompt": prompt,
        "images": stored,
    }))?)
}

/// 选择请求指定的端点或默认第一个图片端点。
fn select_endpoint<'a>(
    endpoints: &'a [ModelEndpointConfig],
    requested_id: Option<&str>,
) -> Result<&'a ModelEndpointConfig> {
    let images = endpoints
        .iter()
        .filter(|endpoint| endpoint.kind == ModelEndpointKind::ImageGeneration)
        .collect::<Vec<_>>();
    if let Some(id) = requested_id.filter(|id| !id.trim().is_empty()) {
        return images
            .into_iter()
            .find(|endpoint| endpoint.id == id)
            .context("configured image endpoint was not found");
    }
    images
        .into_iter()
        .next()
        .context("no image generation endpoint configured")
}

/// 把非空字符串参数复制到请求 JSON。
fn add_string_field(target: &mut serde_json::Map<String, Value>, args: &Value, name: &str) {
    if let Some(value) = args
        .get(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        target.insert(name.to_string(), Value::String(value.to_string()));
    }
}

/// 同时提供 OpenAI 图片接口常用的 size 字段。
fn add_string_field_as_size(target: &mut serde_json::Map<String, Value>, args: &Value, name: &str) {
    let Some(value) = args
        .get(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| value.contains('x'))
    else {
        return;
    };
    target.insert("size".to_string(), Value::String(value.to_string()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn endpoint(id: &str) -> ModelEndpointConfig {
        ModelEndpointConfig {
            id: id.into(),
            kind: ModelEndpointKind::ImageGeneration,
            name: id.into(),
            endpoint: "https://example.com/images".into(),
            api_key: String::new(),
            model: "image-model".into(),
        }
    }

    #[test]
    fn selects_requested_image_endpoint() {
        let endpoints = [endpoint("first"), endpoint("second")];
        let selected = select_endpoint(&endpoints, Some("second")).unwrap();
        assert_eq!(selected.id, "second");
    }

    #[tokio::test]
    async fn requests_base64_image_and_writes_cache() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let mut png = Vec::new();
        image::DynamicImage::new_rgb8(1, 1)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let encoded = base64::engine::general_purpose::STANDARD.encode(png);
        let body = serde_json::to_vec(&json!({"data": [{"b64_json": encoded}]})).unwrap();
        let task = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = vec![0; 4096];
            let length = stream.read(&mut request).await.unwrap();
            let request = String::from_utf8_lossy(&request[..length]);
            assert!(request.contains("\"prompt\":\"a red square\""));
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(header.as_bytes()).await.unwrap();
            stream.write_all(&body).await.unwrap();
        });
        let mut configured = endpoint("local");
        configured.endpoint = format!("http://{address}");
        let temp = tempfile::tempdir().unwrap();
        let (progress_tx, mut progress_rx) = tokio::sync::mpsc::unbounded_channel();
        let output = generate(
            json!({"prompt": "a red square", "resolution": "1024x1024"}),
            vec![configured],
            temp.path().join("generated"),
            ToolProgress::new(progress_tx),
        )
        .await
        .unwrap();
        task.await.unwrap();
        let result: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(result["images"].as_array().unwrap().len(), 1);
        let path = result["images"][0]["local_path"].as_str().unwrap();
        assert!(std::path::Path::new(path).is_file());
        let progress = std::iter::from_fn(|| progress_rx.try_recv().ok()).collect::<Vec<_>>();
        assert!(progress.iter().any(|item| item.contains("图片生成中")));
    }
}
