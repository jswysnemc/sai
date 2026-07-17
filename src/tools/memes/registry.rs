use super::{vision, ToolRegistry, ToolSpec};
use crate::config::{AppConfig, MemesPluginConfig};
use crate::i18n::text as t;
use crate::llm::{ChatMessage, OpenAiCompatibleClient};
use crate::paths::SaiPaths;
use crate::prompts::MEME_DESCRIPTION_PROMPT;
use anyhow::{bail, Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const BUILTIN_MEMES_DIR: &str = "/usr/share/sai/memes";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct MemeIndex {
    #[serde(default)]
    library: String,
    #[serde(default)]
    version: u32,
    #[serde(default)]
    memes: Vec<MemeItem>,
    #[serde(default)]
    disabled_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MemeItem {
    id: String,
    name: LocalizedName,
    file: String,
    mime_type: String,
    #[serde(default)]
    animated: bool,
    description: String,
    usage: String,
    avoid: String,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct LocalizedName {
    #[serde(default)]
    zh: String,
    #[serde(default)]
    en: String,
}

#[derive(Debug, Clone)]
struct LoadedMeme {
    item: MemeItem,
    path: PathBuf,
    source: MemeSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AutoMemeEvent {
    pub library: String,
    pub id: String,
    pub name: Value,
    pub description: String,
    pub usage: String,
    pub reason: String,
    pub sent_at: String,
}

#[derive(Debug, Clone)]
pub(crate) struct AutoMemePlan {
    pub event: AutoMemeEvent,
    pub reminder: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct AutoMemeState {
    #[serde(default)]
    last: Option<AutoMemeEvent>,
}

#[derive(Debug, Deserialize)]
struct AutoSendDecision {
    #[serde(default)]
    send: bool,
    #[serde(default)]
    id: String,
    #[serde(default)]
    confidence: f32,
    #[serde(default)]
    reason: String,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum MemeSource {
    Builtin,
    User,
}

pub fn register(registry: &mut ToolRegistry, config: AppConfig, paths: SaiPaths) {
    if !config.plugins.memes.enabled {
        return;
    }
    registry.register(ToolSpec::new(
        "search_meme",
        t(
            "Search the current persona's meme library by scene, mood, tags, or visible content. Use before showing a meme unless the user provided a specific meme id.",
            "按场景、情绪、标签或画面内容搜索当前人格表情库。除非用户给了具体表情 id，否则发表情前先调用。",
        ),
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": t("Scene, mood, visible content, or user intent.", "场景、情绪、画面内容或用户意图。") },
                "tags": { "type": "array", "items": { "type": "string" }, "description": t("Optional preferred tags.", "可选偏好标签。") },
                "library": { "type": "string", "description": t("Optional meme library override.", "可选表情库覆盖。") },
                "limit": { "type": "integer", "description": t("Maximum number of candidates, default 6.", "候选数量上限，默认 6。") }
            },
            "additionalProperties": false
        }),
        {
            let config = config.clone();
            let paths = paths.clone();
            move |args| {
                let config = config.clone();
                let paths = paths.clone();
                async move { search_meme(args, &config, &paths).await }
            }
        },
    ));
    registry.register(ToolSpec::new(
        "show_meme",
        t(
            "Render a meme in the terminal with terminal image protocols or an ANSI fallback. GIFs are shown as static previews unless animation is explicitly allowed in config.",
            "发送表情包并使用终端图片协议或 ANSI 降级渲染。GIF 默认显示静态预览，除非配置显式允许动画。",
        ),
        json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": t("Meme sha256 id.", "表情 sha256 id。") },
                "library": { "type": "string", "description": t("Optional meme library override.", "可选表情库覆盖。") },
                "size": { "type": "string", "description": t("Optional terminal size, e.g. 40x15.", "可选终端显示尺寸，例如 40x15。") },
                "width": { "type": "integer", "description": t("Optional output width in terminal cells.", "可选终端单元格输出宽度。") },
                "height": { "type": "integer", "description": t("Optional output height in terminal cells.", "可选终端单元格输出高度。") }
            },
            "required": ["id"],
            "additionalProperties": false
        }),
        {
            let config = config.clone();
            let paths = paths.clone();
            move |args| {
                let config = config.clone();
                let paths = paths.clone();
                async move { show_meme(args, &config, &paths).await }
            }
        },
    ));
    registry.register(ToolSpec::new(
        "recent_meme",
        t(
            "Get the most recent meme automatically sent for the current persona/library.",
            "查询当前人格/表情库最近一次自动发送的表情。",
        ),
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        }),
        {
            let config = config.clone();
            let paths = paths.clone();
            move |_| {
                let config = config.clone();
                let paths = paths.clone();
                async move { recent_meme(&config, &paths).await }
            }
        },
    ));
    registry.register(
        ToolSpec::new(
            "add_meme",
            t(
                "Add a local image to the current persona's writable meme library. If metadata is not supplied, the tool asks the configured vision model to generate it from the image.",
                "把本地图片加入当前人格的可写表情库。若未提供元数据，工具会调用配置的识图模型根据图片生成。",
            ),
            json!({
                "type": "object",
                "properties": {
                    "image": { "type": "string", "description": t("Local image path.", "本地图片路径。") },
                    "library": { "type": "string", "description": t("Optional meme library override.", "可选表情库覆盖。") },
                    "name_zh": { "type": "string", "description": t("Chinese display name.", "中文显示名。") },
                    "name_en": { "type": "string", "description": t("English display name.", "英文显示名。") },
                    "description": { "type": "string", "description": t("Visible content description.", "图片可见内容描述。") },
                    "usage": { "type": "string", "description": t("When to use this meme.", "什么时候使用该表情。") },
                    "avoid": { "type": "string", "description": t("When not to use this meme.", "什么场景不要使用。") },
                    "tags": { "type": "array", "items": { "type": "string" }, "description": t("Search tags.", "检索标签。") }
                },
                "required": ["image"],
                "additionalProperties": false
            }),
            {
                let config = config.clone();
                let paths = paths.clone();
                move |args| {
                    let config = config.clone();
                    let paths = paths.clone();
                    async move { add_meme(args, &config, &paths).await }
                }
            },
        )
        .writes(),
    );
    registry.register(
        ToolSpec::new(
            "update_meme",
            t(
                "Update meme index metadata in the writable overlay for the current library.",
                "更新当前表情库可写覆盖层中的表情元数据。",
            ),
            json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": t("Meme sha256 id.", "表情 sha256 id。") },
                    "library": { "type": "string", "description": t("Optional meme library override.", "可选表情库覆盖。") },
                    "name_zh": { "type": "string" },
                    "name_en": { "type": "string" },
                    "description": { "type": "string" },
                    "usage": { "type": "string" },
                    "avoid": { "type": "string" },
                    "tags": { "type": "array", "items": { "type": "string" } },
                    "enabled": { "type": "boolean", "description": t("Enable or disable this meme.", "启用或禁用该表情。") }
                },
                "required": ["id"],
                "additionalProperties": false
            }),
            {
                let config = config.clone();
                let paths = paths.clone();
                move |args| {
                    let config = config.clone();
                    let paths = paths.clone();
                    async move { update_meme(args, &config, &paths).await }
                }
            },
        )
        .writes(),
    );
    registry.register(
        ToolSpec::new(
            "delete_meme",
            t(
                "Delete a user meme or disable a built-in meme in the current library.",
                "删除用户表情，或在当前表情库中禁用内置表情。",
            ),
            json!({
                "type": "object",
                "properties": {
                    "id": { "type": "string", "description": t("Meme sha256 id.", "表情 sha256 id。") },
                    "library": { "type": "string", "description": t("Optional meme library override.", "可选表情库覆盖。") },
                    "hard_delete": { "type": "boolean", "description": t("Permanently remove user image instead of moving it to trash.", "永久删除用户图片，而不是移入回收站。") }
                },
                "required": ["id"],
                "additionalProperties": false
            }),
            {
                let config = config.clone();
                let paths = paths.clone();
                move |args| {
                    let config = config.clone();
                    let paths = paths.clone();
                    async move { delete_meme(args, &config, &paths).await }
                }
            },
        )
        .writes(),
    );
}

