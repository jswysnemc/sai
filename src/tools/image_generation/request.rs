use crate::config::ModelEndpointConfig;
use anyhow::Result;
use serde_json::{json, Value};

/// 生图请求支持的协议族。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ImageProtocol {
    OpenAiImages,
    Gemini,
}

/// 生图请求的认证方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ImageAuth {
    Bearer,
    GeminiQueryKey,
}

/// 已完成地址、请求体和认证方式适配的图片请求。
#[derive(Debug, Clone)]
pub(super) struct PreparedImageRequest {
    pub(super) url: String,
    pub(super) body: Value,
    pub(super) auth: ImageAuth,
}

/// 根据端点配置和请求参数构造图片请求。
pub(super) fn prepare_generation_request(
    endpoint: &ModelEndpointConfig,
    prompt: &str,
    aspect_ratio: Option<&str>,
    resolution: Option<&str>,
    images: &[String],
) -> Result<PreparedImageRequest> {
    let protocol = resolve_protocol(endpoint);
    match protocol {
        ImageProtocol::OpenAiImages => Ok(PreparedImageRequest {
            url: openai_generation_url(&endpoint.endpoint),
            body: openai_body(endpoint, prompt, aspect_ratio, resolution),
            auth: ImageAuth::Bearer,
        }),
        ImageProtocol::Gemini => {
            let model = endpoint.model.trim();
            if model.is_empty() {
                anyhow::bail!("Gemini image endpoint requires a model");
            }
            Ok(PreparedImageRequest {
                url: gemini_generation_url(&endpoint.endpoint, model),
                body: gemini_body(prompt, aspect_ratio, resolution, images),
                auth: ImageAuth::GeminiQueryKey,
            })
        }
    }
}

/// 根据手动协议或端点特征选择协议。
pub(super) fn resolve_protocol(endpoint: &ModelEndpointConfig) -> ImageProtocol {
    let configured = endpoint.protocol.trim().to_ascii_lowercase();
    if matches!(configured.as_str(), "gemini" | "google-gemini") {
        return ImageProtocol::Gemini;
    }
    if matches!(
        configured.as_str(),
        "openai" | "openai-images" | "openai_image"
    ) {
        return ImageProtocol::OpenAiImages;
    }
    let endpoint_url = endpoint.endpoint.to_ascii_lowercase();
    let model = endpoint.model.to_ascii_lowercase();
    if endpoint_url.contains("generativelanguage.googleapis.com")
        || endpoint_url.contains("generatecontent")
        || model.starts_with("gemini")
    {
        ImageProtocol::Gemini
    } else {
        ImageProtocol::OpenAiImages
    }
}

/// 生成模型目录地址和认证方式。
pub(crate) fn model_catalog_url(endpoint: &ModelEndpointConfig) -> (String, ImageAuth) {
    match resolve_protocol(endpoint) {
        ImageProtocol::OpenAiImages => (openai_models_url(&endpoint.endpoint), ImageAuth::Bearer),
        ImageProtocol::Gemini => (
            gemini_models_url(&endpoint.endpoint),
            ImageAuth::GeminiQueryKey,
        ),
    }
}

fn openai_body(
    endpoint: &ModelEndpointConfig,
    prompt: &str,
    aspect_ratio: Option<&str>,
    resolution: Option<&str>,
) -> Value {
    let mut body = serde_json::Map::new();
    if !endpoint.model.trim().is_empty() {
        body.insert(
            "model".into(),
            Value::String(endpoint.model.trim().to_string()),
        );
    }
    body.insert("prompt".into(), Value::String(prompt.to_string()));
    body.insert("n".into(), Value::Number(1.into()));
    if let Some(value) = aspect_ratio.filter(|value| !value.trim().is_empty()) {
        body.insert(
            "aspect_ratio".into(),
            Value::String(value.trim().to_string()),
        );
    }
    if let Some(value) = resolution.filter(|value| !value.trim().is_empty()) {
        body.insert("resolution".into(), Value::String(value.trim().to_string()));
        if value.contains('x') {
            body.insert("size".into(), Value::String(value.trim().to_string()));
        }
    }
    Value::Object(body)
}

fn gemini_body(
    prompt: &str,
    aspect_ratio: Option<&str>,
    resolution: Option<&str>,
    images: &[String],
) -> Value {
    let mut image_config = serde_json::Map::new();
    if let Some(value) = aspect_ratio.filter(|value| !value.trim().is_empty()) {
        image_config.insert(
            "aspectRatio".into(),
            Value::String(value.trim().to_string()),
        );
    }
    if let Some(value) = resolution.filter(|value| !value.trim().is_empty()) {
        image_config.insert("imageSize".into(), Value::String(gemini_image_size(value)));
    }
    let mut generation_config = serde_json::Map::new();
    generation_config.insert("responseModalities".into(), json!(["TEXT", "IMAGE"]));
    if !image_config.is_empty() {
        generation_config.insert("imageConfig".into(), Value::Object(image_config));
    }
    let mut parts = inline_image_parts(images);
    parts.push(json!({ "text": prompt }));
    json!({
        "contents": [{"parts": parts}],
        "generationConfig": generation_config
    })
}

/// 把 data URL 收成 Gemini 能读的内联图片，最多四张。
fn inline_image_parts(images: &[String]) -> Vec<Value> {
    images
        .iter()
        .filter_map(|value| split_data_url(value))
        .take(4)
        .map(|(mime, data)| {
            json!({ "inline_data": { "mime_type": format!("image/{mime}"), "data": data } })
        })
        .collect()
}

/// 拆开 `data:image/png;base64,...`。不是图片 data URL 时返回空。
fn split_data_url(value: &str) -> Option<(&str, &str)> {
    let rest = value.strip_prefix("data:image/")?;
    let (mime, data) = rest.split_once(";base64,")?;
    if mime.is_empty() || data.is_empty() {
        return None;
    }
    Some((mime, data))
}

fn gemini_image_size(resolution: &str) -> String {
    let is_2k = resolution
        .split_once('x')
        .and_then(|(width, height)| {
            let width = width.parse::<u32>().ok()?;
            let height = height.parse::<u32>().ok()?;
            Some(width.max(height) >= 1536)
        })
        .unwrap_or_else(|| resolution.starts_with("2K") || resolution.starts_with("2048"));
    if is_2k {
        "2K".to_string()
    } else {
        "1K".to_string()
    }
}

fn openai_generation_url(endpoint: &str) -> String {
    append_openai_suffix(endpoint, "/images/generations")
}

fn openai_models_url(endpoint: &str) -> String {
    let base = strip_known_generation_suffix(endpoint);
    append_openai_suffix(&base, "/models")
}

fn append_openai_suffix(endpoint: &str, suffix: &str) -> String {
    let value = endpoint.trim().trim_end_matches('/');
    let lower = value.to_ascii_lowercase();
    if lower.ends_with(suffix)
        || lower.ends_with("/images/generate")
        || lower.ends_with("/generate")
    {
        return value.to_string();
    }
    if lower.ends_with("/v1")
        || lower.ends_with("/v1beta")
        || lower.ends_with("/openai")
        || lower.ends_with("/api")
        || reqwest::Url::parse(value)
            .ok()
            .is_some_and(|url| url.path() == "/")
    {
        return format!("{value}{suffix}");
    }
    value.to_string()
}

fn strip_known_generation_suffix(endpoint: &str) -> String {
    let value = endpoint.trim().trim_end_matches('/');
    [
        "/images/generations",
        "/images/generate",
        "/generate",
        "/images",
    ]
    .iter()
    .find_map(|suffix| {
        value
            .to_ascii_lowercase()
            .ends_with(suffix)
            .then(|| value[..value.len() - suffix.len()].to_string())
    })
    .unwrap_or_else(|| value.to_string())
}

fn gemini_generation_url(endpoint: &str, model: &str) -> String {
    let value = endpoint.trim().trim_end_matches('/');
    let lower = value.to_ascii_lowercase();
    if lower.contains(":generatecontent") {
        return value.to_string();
    }
    let model = model.strip_prefix("models/").unwrap_or(model);
    if lower.contains("/models/") {
        return format!("{value}:generateContent");
    }
    format!("{value}/models/{model}:generateContent")
}

fn gemini_models_url(endpoint: &str) -> String {
    let value = endpoint.trim().trim_end_matches('/');
    let lower = value.to_ascii_lowercase();
    let base = lower
        .find("/models/")
        .map(|index| &value[..index])
        .unwrap_or(value);
    format!("{}/models", base.trim_end_matches('/'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ModelEndpointKind;

    fn endpoint(url: &str, protocol: &str, model: &str) -> ModelEndpointConfig {
        ModelEndpointConfig {
            id: "image".into(),
            kind: ModelEndpointKind::ImageGeneration,
            name: "Image".into(),
            endpoint: url.into(),
            protocol: protocol.into(),
            api_key: "key".into(),
            api_keys: Vec::new(),
            api_key_selected: None,
            api_key_balance: false,
            models: Vec::new(),
            model: model.into(),
        }
    }

    #[test]
    fn adapts_openai_base_url_to_generation_path() {
        let request = prepare_generation_request(
            &endpoint("https://image.example/v1", "auto", "gpt-image-1"),
            "a red square",
            Some("1:1"),
            Some("1024x1024"),
            &[],
        )
        .unwrap();
        assert_eq!(request.url, "https://image.example/v1/images/generations");
        assert_eq!(request.body["size"], "1024x1024");
        assert_eq!(request.auth, ImageAuth::Bearer);
    }

    #[test]
    fn builds_gemini_generate_content_request() {
        let request = prepare_generation_request(
            &endpoint(
                "https://generativelanguage.googleapis.com/v1beta",
                "auto",
                "gemini-2.0-flash-exp",
            ),
            "a red square",
            Some("16:9"),
            Some("2048x1152"),
            &[],
        )
        .unwrap();
        assert_eq!(request.url, "https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash-exp:generateContent");
        assert_eq!(
            request.body["generationConfig"]["imageConfig"]["aspectRatio"],
            "16:9"
        );
        assert_eq!(
            request.body["generationConfig"]["imageConfig"]["imageSize"],
            "2K"
        );
        assert_eq!(request.auth, ImageAuth::GeminiQueryKey);
    }

    #[test]
    fn maps_portrait_dimensions_to_the_same_gemini_size_as_landscape() {
        assert_eq!(gemini_image_size("1536x1152"), "2K");
        assert_eq!(gemini_image_size("1152x1536"), "2K");
        assert_eq!(gemini_image_size("864x1536"), "2K");
    }

    #[test]
    fn gemini_request_includes_inline_reference_images() {
        let request = prepare_generation_request(
            &endpoint(
                "https://generativelanguage.googleapis.com/v1beta",
                "gemini",
                "gemini-2.0-flash-exp",
            ),
            "make it night",
            None,
            None,
            &["data:image/png;base64,aaaa".to_string()],
        )
        .unwrap();
        assert_eq!(
            request.body["contents"][0]["parts"][0]["inline_data"]["mime_type"],
            "image/png"
        );
        assert_eq!(
            request.body["contents"][0]["parts"][0]["inline_data"]["data"],
            "aaaa"
        );
        assert_eq!(request.body["contents"][0]["parts"][1]["text"], "make it night");
    }
}
