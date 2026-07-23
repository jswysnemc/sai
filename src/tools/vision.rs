use super::{ToolRegistry, ToolSpec};
use crate::config::{AppConfig, PrintImagePluginConfig, ProviderConfig, VisionPluginConfig};
use crate::default_models::{OPENCODE_DEFAULT_VISION_MODEL, OPENCODE_PROVIDER_ID};
use crate::i18n::text as t;
use crate::llm::{ChatMessage, OpenAiCompatibleClient};
use crate::paths::SaiPaths;
use crate::render::terminal_image;
use anyhow::{bail, Context, Result};
use base64::Engine;
use serde_json::{json, Value};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;

pub fn register_print(registry: &mut ToolRegistry, config: AppConfig) {
    if !config.plugins.print_image.enabled {
        return;
    }
    registry.register(ToolSpec::new(
        "print_image",
        t("Print/render a local image directly in the current terminal output using terminal image protocols or an ANSI fallback. Use this when the user asks to show, print, render, or preview an image, or when you need to inspect an image visually in the terminal before answering.", "使用终端图片协议或 ANSI 降级在当前终端输出中直接打印/渲染本地图片。当用户要求显示、打印、渲染、预览图片，或回答前需要在终端中目视检查图片时使用。"),
        json!({
            "type": "object",
            "properties": {
                "image": { "type": "string", "description": t("Local image path.", "本地图片路径。") },
                "size": { "type": "string", "description": t("Optional terminal size, e.g. 80x40. Use this or width/height to avoid oversized output.", "可选终端显示尺寸，例如 80x40。用它或 width/height 避免输出过大。") },
                "width": { "type": "integer", "description": t("Optional output width in terminal cells, e.g. 80.", "可选终端单元格输出宽度，例如 80。") },
                "height": { "type": "integer", "description": t("Optional output height in terminal cells, e.g. 40.", "可选终端单元格输出高度，例如 40。") }
            },
            "required": ["image"],
            "additionalProperties": false
        }),
        move |args| {
            let print_config = config.plugins.print_image.clone();
            async move { print_image(args, &print_config).await }
        },
    ));
}

async fn print_image(args: Value, print_config: &PrintImagePluginConfig) -> Result<String> {
    let image = args
        .get("image")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if image.is_empty() {
        bail!("{}", t("image is required", "缺少图片路径"))
    }
    let path = expand_path(image);
    let metadata = std::fs::metadata(&path).with_context(|| {
        format!(
            "{} {}",
            t("failed to stat image", "无法读取图片元数据"),
            path.display()
        )
    })?;
    if !metadata.is_file() {
        bail!(
            "{}: {}",
            t("image path is not a file", "图片路径不是文件"),
            path.display()
        )
    }
    print_image_file(&path, print_size(&args, print_config)).await?;
    Ok(format!(
        "{}: {}",
        t("printed image in terminal", "已在终端打印图片"),
        path.display()
    ))
}

pub async fn print_image_file(path: &Path, size: Option<String>) -> Result<()> {
    println!();
    io::stdout().flush()?;
    let rendered = terminal_image::render_terminal_image_with_size(path, size.as_deref())
        .with_context(|| format!("failed to render image {}", path.display()))?;
    print!("{rendered}");
    if !rendered.ends_with('\n') {
        println!();
    }
    println!();
    io::stdout().flush()?;
    Ok(())
}

pub fn configured_print_size(print_config: &PrintImagePluginConfig) -> Option<String> {
    let (cols, rows) = crossterm::terminal::size().ok()?;
    let width = ((cols as u32 * print_config.width_percent as u32) / 100).max(1);
    let height = ((rows as u32 * print_config.height_percent as u32) / 100).max(1);
    Some(format!("{}x{}", width.min(300), height.min(200)))
}

fn print_size(args: &Value, print_config: &PrintImagePluginConfig) -> Option<String> {
    let width = args
        .get("width")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .min(300);
    let height = args
        .get("height")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .min(200);
    match (width, height) {
        (0, 0) => args
            .get("size")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .or_else(|| configured_print_size(print_config)),
        (width, 0) => Some(format!("{width}x")),
        (0, height) => Some(format!("x{height}")),
        (width, height) => Some(format!("{width}x{height}")),
    }
}

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
        config.provider(Some(OPENCODE_PROVIDER_ID))?.clone()
    };
    provider.default_model = if !model.is_empty() {
        model.to_string()
    } else if provider_id.is_empty() {
        OPENCODE_DEFAULT_VISION_MODEL.to_string()
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
    if metadata.len() as usize > MAX_IMAGE_BYTES {
        bail!("image too large: {} bytes", metadata.len())
    }
    let bytes =
        std::fs::read(path).with_context(|| format!("failed to read image {}", path.display()))?;
    let mime = mime_from_path(path)?;
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    Ok(format!("data:{mime};base64,{encoded}"))
}

fn expand_path(value: &str) -> PathBuf {
    let value = value.trim();
    if let Some(rest) = value.strip_prefix("~/") {
        if let Some(home) = directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf()) {
            return home.join(rest);
        }
    }
    let path = Path::new(value);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        crate::runtime_cwd::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
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
