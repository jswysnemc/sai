async fn screen_image_with_vision(
    config: &AppConfig,
    paths: &SaiPaths,
    query: &str,
    item: &StoredImage,
) -> VisionScreening {
    if !vision_screening_available(config) {
        return VisionScreening::not_requested();
    }
    let provider = match vision_provider(config, &config.plugins.vision) {
        Ok(provider) => provider,
        Err(err) => return VisionScreening::failed(err.to_string(), None),
    };
    let client = match OpenAiCompatibleClient::new(&provider, config, paths) {
        Ok(client) => client,
        Err(err) => return VisionScreening::failed(err.to_string(), Some(&provider)),
    };
    let image_url = match local_image_data_url(&item.local_path, item.size_bytes) {
        Ok(value) => value,
        Err(err) => return VisionScreening::failed(err.to_string(), Some(&provider)),
    };
    let prompt = image_screening_prompt(query, &item.candidate);
    let result = client
        .chat_stream(
            vec![
                ChatMessage::system(
                    "你是图片搜索结果筛选器。只根据图片实际内容判断是否匹配用户想看的图片。",
                ),
                ChatMessage::user_with_image(prompt, image_url),
            ],
            Vec::new(),
            |_| Ok(()),
        )
        .await;
    match result {
        Ok(result) => parse_vision_screening(&result.content, &provider),
        Err(err) => VisionScreening::failed(err.to_string(), Some(&provider)),
    }
}

fn vision_screening_available(config: &AppConfig) -> bool {
    config.plugins.web_images.vision_screening_enabled && config.plugins.vision.enabled
}

fn vision_provider(config: &AppConfig, vision: &VisionPluginConfig) -> Result<ProviderConfig> {
    let provider_id = vision.vision_provider_id.trim();
    let model = vision.vision_model.trim();
    let mut provider = if !provider_id.is_empty() {
        config.provider(Some(provider_id))?.clone()
    } else {
        config.provider(Some(OPENCODE_PROVIDER_ID))?.clone()
    };
    provider.default_model = if !model.is_empty() {
        model.to_string()
    } else if provider_id.is_empty() {
        OPENCODE_DEFAULT_VISION_MODEL.to_string()
    } else {
        provider.default_model.clone()
    };
    if provider.default_model.trim().is_empty() {
        bail!("vision provider has no active model")
    }
    if !provider
        .models
        .iter()
        .any(|item| item == &provider.default_model)
    {
        provider.models.push(provider.default_model.clone());
    }
    Ok(provider)
}

fn image_screening_prompt(query: &str, candidate: &ImageCandidate) -> String {
    format!(
        "用户想看的图片：{query}\n搜索结果标题：{}\n搜索结果来源：{}\n搜索结果描述：{}\n\n请判断这张已下载图片是否适合作为用户要看的图片。只输出 JSON，不要 Markdown，不要解释到 JSON 外面。格式：{{\"accepted\": true, \"description\": \"用中文客观描述图片内容\", \"reason\": \"接受或拒绝原因\"}}",
        candidate.title, candidate.page_url, candidate.search_description
    )
}

fn parse_vision_screening(text: &str, provider: &ProviderConfig) -> VisionScreening {
    let raw = text.trim();
    let json_text = raw
        .find('{')
        .and_then(|start| raw.rfind('}').map(|end| &raw[start..=end]));
    if let Some(json_text) = json_text {
        if let Ok(data) = serde_json::from_str::<Value>(json_text) {
            if data.is_object() {
                return VisionScreening {
                    status: "success".to_string(),
                    accepted: parse_boolish(data.get("accepted")).unwrap_or(true),
                    description: data
                        .get("description")
                        .or_else(|| data.get("caption"))
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .trim()
                        .to_string(),
                    reason: data
                        .get("reason")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .trim()
                        .to_string(),
                    provider_id: provider.id.clone(),
                    model: provider.default_model.clone(),
                    error: String::new(),
                };
            }
        }
    }
    VisionScreening {
        status: "success".to_string(),
        accepted: true,
        description: clean_text(raw, 1600),
        reason: "vision model did not return JSON; kept image".to_string(),
        provider_id: provider.id.clone(),
        model: provider.default_model.clone(),
        error: String::new(),
    }
}

fn parse_boolish(value: Option<&Value>) -> Option<bool> {
    match value? {
        Value::Bool(value) => Some(*value),
        Value::String(value) => {
            let lower = value.trim().to_ascii_lowercase();
            Some(!matches!(
                lower.as_str(),
                "false" | "0" | "no" | "reject" | "rejected" | "不" | "否" | "拒绝"
            ))
        }
        Value::Number(value) => Some(value.as_i64().unwrap_or(1) != 0),
        _ => None,
    }
}

