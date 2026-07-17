use super::{vision, ToolProgress, ToolRegistry, ToolSpec};
use crate::config::{AppConfig, ProviderConfig, VisionPluginConfig};
use crate::default_models::{OPENCODE_DEFAULT_VISION_MODEL, OPENCODE_PROVIDER_ID};
use crate::i18n::text as t;
use crate::llm::{ChatMessage, OpenAiCompatibleClient};
use crate::paths::SaiPaths;
use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36";

#[derive(Debug, Clone)]
struct ImageCandidate {
    title: String,
    page_url: String,
    image_url: String,
    thumbnail_url: String,
    source: String,
    width: u32,
    height: u32,
    search_description: String,
}

struct StoredImage {
    candidate: ImageCandidate,
    local_path: PathBuf,
    mime_type: String,
    size_bytes: usize,
    sha256: String,
    used_thumbnail: bool,
    vision: VisionScreening,
}

#[derive(Debug, Clone)]
struct VisionScreening {
    status: String,
    accepted: bool,
    description: String,
    reason: String,
    provider_id: String,
    model: String,
    error: String,
}

impl VisionScreening {
    fn not_requested() -> Self {
        Self {
            status: "not_requested".to_string(),
            accepted: true,
            description: String::new(),
            reason: String::new(),
            provider_id: String::new(),
            model: String::new(),
            error: String::new(),
        }
    }

    fn failed(error: impl Into<String>, provider: Option<&ProviderConfig>) -> Self {
        Self {
            status: "failed".to_string(),
            accepted: true,
            description: String::new(),
            reason: String::new(),
            provider_id: provider.map(|item| item.id.clone()).unwrap_or_default(),
            model: provider
                .map(|item| item.default_model.clone())
                .unwrap_or_default(),
            error: error.into(),
        }
    }
}

pub fn register(
    registry: &mut ToolRegistry,
    config: AppConfig,
    paths: SaiPaths,
    allow_download: bool,
) {
    registry.register(ToolSpec::new_with_progress(
        "search_web_images",
        t(
            "Search web images with DuckDuckGo and Bing fallback. In normal mode it can download selected images to the local cache and optionally preview them in the terminal. In read-only mode it only returns remote image metadata.",
            "搜索网络图片，使用 DuckDuckGo，失败或不足时回退 Bing。普通模式可下载选中图片到本地缓存并可在终端预览；只读模式只返回远程图片元数据。",
        ),
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": t("Image search query.", "图片搜索关键词。") },
                "count": { "type": "integer", "description": t("Required. Exact number of images to return. Match the user's requested quantity: one/a/an/一张/一幅 means 1; a few/几张 means 3; several/多张 means 5 unless the user gives another number. Do not use the configured maximum as the default.", "必填。最终返回图片的精确数量。必须匹配用户要求的数量：一张/一幅/one/a/an 填 1；几张填 3；多张填 5，除非用户给了其他数字。不要把配置上限当默认值。") },
                "preview": { "type": "boolean", "description": t("Download and preview images when terminal image printing is enabled.", "在终端图片打印启用时，下载并预览图片。") },
                "preview_count": { "type": "integer", "description": t("Maximum images to preview in the terminal.", "最多在终端预览几张图片。") },
                "safe_search": { "type": "boolean", "description": t("Enable safe image search. Defaults to plugin config.", "启用安全搜图。默认使用插件配置。") }
            },
            "required": ["query", "count"],
            "additionalProperties": false
        }),
        move |args, progress| {
            let config = config.clone();
            let paths = paths.clone();
            async move { search_web_images(args, config, paths, allow_download, progress).await }
        },
    ));
}

async fn search_web_images(
    args: Value,
    config: AppConfig,
    paths: SaiPaths,
    allow_download: bool,
    progress: ToolProgress,
) -> Result<String> {
    let plugin = &config.plugins.web_images;
    if !plugin.enabled {
        bail!("web image search plugin is disabled")
    }
    let query = args
        .get("query")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if query.is_empty() {
        bail!("query is required")
    }
    let Some(count) = args.get("count").and_then(Value::as_u64) else {
        bail!("count is required; choose the number of images from the user's request")
    };
    let count = count.clamp(1, plugin.max_results.max(1).min(10) as u64) as usize;
    let safe_search = args
        .get("safe_search")
        .and_then(Value::as_bool)
        .unwrap_or(plugin.safe_search);
    let preview = allow_download
        && args
            .get("preview")
            .and_then(Value::as_bool)
            .unwrap_or(plugin.auto_preview);
    let preview_count = args
        .get("preview_count")
        .and_then(Value::as_u64)
        .unwrap_or(count as u64)
        .clamp(0, count.min(5) as u64) as usize;
    let client = Client::builder()
        .timeout(Duration::from_secs(plugin.timeout_seconds.max(5)))
        .redirect(reqwest::redirect::Policy::limited(8))
        .build()?;
    progress.report(t("searching image candidates", "正在搜索图片候选"));
    let candidates = search_images(&client, query, count, safe_search).await?;
    if !allow_download {
        return Ok(json!({
            "success": !candidates.is_empty(),
            "query": query,
            "count": candidates.len().min(count),
            "mode": "metadata_only",
            "images": candidates.into_iter().take(count).map(candidate_json).collect::<Vec<_>>(),
        })
        .to_string());
    }
    let cache_dir = paths.pictures_dir.join("web-images");
    let download_result = download_and_store_images(
        &config,
        &paths,
        &client,
        &cache_dir,
        query,
        candidates,
        count,
        (plugin.max_download_mb.max(0.1) * 1024.0 * 1024.0) as usize,
        progress.clone(),
    )
    .await?;
    let stored = download_result.images;
    let mut print_errors = Vec::new();
    let should_print = preview && config.plugins.print_image.enabled && preview_count > 0;
    if should_print {
        progress.report("__external_output__");
        for item in stored.iter().take(preview_count) {
            if let Err(err) = vision::print_image_file(
                &item.local_path,
                vision::configured_print_size(&config.plugins.print_image),
            )
            .await
            {
                print_errors.push(format!("{}: {err}", item.local_path.display()));
            }
        }
    }
    Ok(json!({
        "success": !stored.is_empty(),
        "query": query,
        "count": stored.len(),
        "result_role": "downloaded_image_candidates",
        "vision_screening": if vision_screening_available(&config) { "enabled" } else { "unavailable" },
        "description_policy": "vision.description is produced by the configured vision model after download; search_description is only search-engine metadata. Prefer vision.description when explaining whether an image matches the request.",
        "rejected_by_vision": download_result.rejected_by_vision,
        "cache_dir": cache_dir,
        "printed": should_print && print_errors.is_empty() && !stored.is_empty(),
        "print_errors": print_errors,
        "images": stored.into_iter().map(stored_json).collect::<Vec<_>>(),
        "assistant_instruction": if should_print {
            "The searched images have been downloaded and previewed in the terminal when possible. In your final response, include the local_path values for reusable images. Do not call print_image again for already printed images unless the user asks."
        } else {
            "The searched images have been downloaded to local_path. In your final response, include useful local_path and page_url values. Call print_image only if the user explicitly asks to render or preview them."
        }
    })
    .to_string())
}

struct DownloadResult {
    images: Vec<StoredImage>,
    rejected_by_vision: usize,
}

