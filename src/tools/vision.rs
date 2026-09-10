use crate::config::{AppConfig, ProviderConfig, VisionPluginConfig};
use crate::llm::{ChatMessage, OpenAiCompatibleClient};
use crate::paths::SaiPaths;
use anyhow::{bail, Context, Result};
use base64::Engine;
use std::path::Path;

pub async fn analyze_local_image_with_prompt(
    config: &AppConfig,
    paths: &SaiPaths,
    image: &Path,
    prompt: &str,
) -> Result<String> {
    let image_url = local_image_data_url(image)?;
    analyze_image_url_with_prompt(config, paths, &image_url, prompt).await
}

async fn analyze_image_url_with_prompt(
    config: &AppConfig,
    paths: &SaiPaths,
    image_url: &str,
    prompt: &str,
) -> Result<String> {
    let vision = &config.plugins.vision;
    if !vision.enabled {
        bail!("vision plugin is disabled")
    }
    let provider = vision_provider(&config, vision)?;
    let client = OpenAiCompatibleClient::new(&provider, &config, &paths)?;
    let result = client
        .chat_stream(
            vec![
                ChatMessage::system("请基于图片内容回答，不要编造看不见的信息。"),
                ChatMessage::user_with_image(prompt, image_url.to_string()),
            ],
            Vec::new(),
            |_| Ok(()),
        )
        .await?;
    if result.content.trim().is_empty() {
        bail!("vision model returned empty response")
    }
    Ok(result.content)
}

fn vision_provider(config: &AppConfig, vision: &VisionPluginConfig) -> Result<ProviderConfig> {
    let provider_id = vision.vision_provider_id.trim();
    let model = vision.vision_model.trim();
    let mut provider = if !provider_id.is_empty() {
        config.provider(Some(provider_id))?.clone()
    } else {
        config.provider(None)?.clone()
    };
    provider.default_model = if !model.is_empty() {
        model.to_string()
    } else {
        provider.default_model.clone()
    };
    if !provider
        .models
        .iter()
        .any(|item| item == &provider.default_model)
    {
        provider.models.push(provider.default_model.clone());
    }
    Ok(provider)
}

/// 将本地图片编码为模型请求使用的 data URL。
///
/// 参数:
/// - `path`: 本地图片路径
///
/// 返回:
/// - 包含 MIME 类型和 Base64 数据的 URL
pub(crate) fn local_image_data_url(path: &Path) -> Result<String> {
    let metadata = std::fs::metadata(path)
        .with_context(|| format!("failed to stat image {}", path.display()))?;
    if !metadata.is_file() {
        bail!("image path is not a file: {}", path.display())
    }
    let bytes =
        std::fs::read(path).with_context(|| format!("failed to read image {}", path.display()))?;
    let mime = mime_from_path(path)?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    Ok(format!("data:{mime};base64,{encoded}"))
}

fn mime_from_path(path: &Path) -> Result<&'static str> {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => Ok("image/jpeg"),
        "png" => Ok("image/png"),
        "webp" => Ok("image/webp"),
        "gif" => Ok("image/gif"),
        value => {
            bail!("unsupported image extension: {value}; supported: jpg, jpeg, png, webp, gif")
        }
    }
}
