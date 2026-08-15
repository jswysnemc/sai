use super::defaults::*;
use super::model::MemoryConfig;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// CLI 助手可选工具的历史兼容配置容器。
///
/// 配置文件继续使用 `plugins` 键，避免破坏既有用户配置；界面统一使用
/// “CLI 助手工具”语义。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginsConfig {
    #[serde(default)]
    pub weather: PluginEnabledConfig,
    #[serde(default)]
    pub web: WebSearchConfig,
    #[serde(default)]
    pub web_images: WebImagesPluginConfig,
    #[serde(default)]
    pub deep_diagnose: DeepDiagnosePluginConfig,
    #[serde(default)]
    pub vision: VisionPluginConfig,
    #[serde(default)]
    pub exchange_rate: ExchangeRatePluginConfig,
    #[serde(default)]
    pub xuanxue: PluginEnabledConfig,
    #[serde(default)]
    pub image_generation: ImageGenerationPluginConfig,
    #[serde(default)]
    pub print_image: PrintImagePluginConfig,
    #[serde(default)]
    pub memes: MemesPluginConfig,
    #[serde(default)]
    pub knowledge_base: KnowledgeBasePluginConfig,
    #[serde(default)]
    pub archlinux: PluginEnabledConfig,
    #[serde(default)]
    pub man: PluginEnabledConfig,
    #[serde(default)]
    pub moegirl: PluginEnabledConfig,
    #[serde(default)]
    pub hash_codec: PluginEnabledConfig,
    #[serde(default)]
    pub calculator: CalculatorPluginConfig,
    #[serde(default)]
    pub package_advisor: PluginEnabledConfig,
    #[serde(default)]
    pub linux_game_compatibility: LinuxGameCompatibilityConfig,
    #[serde(default)]
    pub diagnostics: DiagnosticsPluginConfig,
    #[serde(default)]
    pub memory: MemoryConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginEnabledConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinuxGameCompatibilityConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_subagent_max_tool_steps")]
    pub max_tool_steps: usize,
}

/// Web 搜索及各供应商的详细配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_web_search_provider")]
    pub default_provider: String,
    #[serde(default = "default_web_search_max_results")]
    pub max_results: usize,
    #[serde(default = "default_web_search_timeout")]
    pub timeout_seconds: u64,

    #[serde(default = "default_true")]
    pub tinyfish_enabled: bool,
    #[serde(default)]
    pub tinyfish_api_keys: Vec<String>,
    #[serde(default = "default_tinyfish_base_url")]
    pub tinyfish_base_url: String,
    #[serde(default)]
    pub tinyfish_default_location: String,
    #[serde(default)]
    pub tinyfish_default_language: String,

    #[serde(default = "default_true")]
    pub tavily_enabled: bool,
    #[serde(default)]
    pub tavily_api_keys: Vec<String>,
    #[serde(default = "default_tavily_base_url")]
    pub tavily_base_url: String,
    #[serde(default = "default_tavily_search_depth")]
    pub tavily_search_depth: String,
    #[serde(default)]
    pub tavily_include_answer: bool,
    #[serde(default = "default_true")]
    pub tavily_include_raw_content: bool,

    #[serde(default = "default_true")]
    pub firecrawl_enabled: bool,
    #[serde(default)]
    pub firecrawl_api_keys: Vec<String>,
    #[serde(default = "default_firecrawl_base_url")]
    pub firecrawl_base_url: String,
    #[serde(default = "default_true")]
    pub firecrawl_only_main_content: bool,

    #[serde(default = "default_true")]
    pub anysearch_enabled: bool,
    #[serde(default)]
    pub anysearch_api_keys: Vec<String>,
    #[serde(default = "default_anysearch_base_url")]
    pub anysearch_base_url: String,

    #[serde(default = "default_true")]
    pub searxng_enabled: bool,
    #[serde(default)]
    pub searxng_base_url: String,
    #[serde(default = "default_searxng_language")]
    pub searxng_language: String,
    #[serde(default)]
    pub searxng_safe_search: u8,

    #[serde(default = "default_true")]
    pub duckduckgo_enabled: bool,
}

impl WebSearchConfig {
    /// 归一化历史遗留的搜索服务地址。
    ///
    /// 旧版 TUI 允许写入不带协议前缀的地址（例如 `localhost:8888`），
    /// 直接进入校验会让整份配置无法加载，因此加载阶段先补齐 `https://`
    ///
    /// 返回:
    /// - 无
    pub(crate) fn normalize_endpoints(&mut self) {
        for endpoint in [
            &mut self.tinyfish_base_url,
            &mut self.tavily_base_url,
            &mut self.firecrawl_base_url,
            &mut self.anysearch_base_url,
            &mut self.searxng_base_url,
        ] {
            let trimmed = endpoint.trim();
            if trimmed.is_empty()
                || trimmed.starts_with("http://")
                || trimmed.starts_with("https://")
            {
                *endpoint = trimmed.to_string();
                continue;
            }
            *endpoint = format!("https://{trimmed}");
        }
    }

    ///
    /// 校验 Web 搜索总参数与供应商选项。
    ///
    /// 返回:
    /// - 配置合法时成功，否则返回具体字段错误
    pub(crate) fn validate(&self) -> Result<()> {
        const PROVIDERS: [&str; 7] = [
            "auto",
            "tinyfish",
            "tavily",
            "firecrawl",
            "anysearch",
            "searxng",
            "duckduckgo",
        ];
        if !PROVIDERS.contains(&self.default_provider.as_str()) {
            bail!(
                "plugins.web.default_provider is invalid: {}",
                self.default_provider
            );
        }
        if !(1..=10).contains(&self.max_results) {
            bail!("plugins.web.max_results must be between 1 and 10");
        }
        if !(1..=120).contains(&self.timeout_seconds) {
            bail!("plugins.web.timeout_seconds must be between 1 and 120");
        }
        if !matches!(self.tavily_search_depth.as_str(), "basic" | "advanced") {
            bail!(
                "plugins.web.tavily_search_depth is invalid: {}",
                self.tavily_search_depth
            );
        }
        if self.searxng_safe_search > 2 {
            bail!("plugins.web.searxng_safe_search must be between 0 and 2");
        }
        for (name, endpoint) in [
            ("tinyfish", self.tinyfish_base_url.as_str()),
            ("tavily", self.tavily_base_url.as_str()),
            ("firecrawl", self.firecrawl_base_url.as_str()),
            ("anysearch", self.anysearch_base_url.as_str()),
            ("searxng", self.searxng_base_url.as_str()),
        ] {
            validate_search_endpoint(name, endpoint)?;
        }
        if self.default_provider != "auto" && !self.provider_enabled(&self.default_provider) {
            bail!(
                "plugins.web.default_provider is disabled: {}",
                self.default_provider
            );
        }
        if self.default_provider == "searxng" && self.searxng_base_url.trim().is_empty() {
            bail!("plugins.web.searxng_base_url is required when SearXNG is the default");
        }
        Ok(())
    }

    /// 判断指定搜索供应商是否启用。
    ///
    /// 参数:
    /// - `provider`: 搜索供应商标识
    ///
    /// 返回:
    /// - 供应商启用时返回 true
    pub(crate) fn provider_enabled(&self, provider: &str) -> bool {
        match provider {
            "tinyfish" => self.tinyfish_enabled,
            "tavily" => self.tavily_enabled,
            "firecrawl" => self.firecrawl_enabled,
            "anysearch" => self.anysearch_enabled,
            "searxng" => self.searxng_enabled && !self.searxng_base_url.trim().is_empty(),
            "duckduckgo" | "script" => self.duckduckgo_enabled,
            _ => false,
        }
    }
}

/// 校验可选搜索服务地址。
///
/// 参数:
/// - `provider`: 供应商标识
/// - `endpoint`: 服务地址
///
/// 返回:
/// - 空地址或合法 HTTP(S) 地址返回成功
fn validate_search_endpoint(provider: &str, endpoint: &str) -> Result<()> {
    let endpoint = endpoint.trim();
    if endpoint.is_empty() || endpoint.starts_with("http://") || endpoint.starts_with("https://") {
        return Ok(());
    }
    bail!("plugins.web.{provider}_base_url must start with http:// or https://");
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebImagesPluginConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_web_images_max_results")]
    pub max_results: usize,
    #[serde(default = "default_web_images_max_download_mb")]
    pub max_download_mb: f64,
    #[serde(default = "default_true")]
    pub safe_search: bool,
    #[serde(default = "default_true")]
    pub vision_screening_enabled: bool,
    #[serde(default = "default_true")]
    pub auto_preview: bool,
    #[serde(default = "default_web_images_preview_count")]
    pub preview_count: usize,
    #[serde(default = "default_web_images_timeout")]
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepDiagnosePluginConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_deep_diagnose_depth")]
    pub thinking_depth: String,
    #[serde(default = "default_deep_diagnose_max_review_revisions")]
    pub max_review_revisions: usize,
    #[serde(default = "default_deep_diagnose_max_tool_steps")]
    pub max_tool_steps_per_round: usize,
    #[serde(default)]
    pub max_final_answer_chars: usize,
    #[serde(default = "default_deep_diagnose_tool_timeout")]
    pub tool_call_timeout_seconds: u64,
    #[serde(default = "default_subagent_max_tool_steps")]
    pub max_tool_steps: usize,
    #[serde(default = "default_true")]
    pub show_progress: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionPluginConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub prefer_current_multimodal_model: bool,
    #[serde(default)]
    pub vision_provider_id: String,
    #[serde(default)]
    pub vision_model: String,
    #[serde(default = "default_true")]
    pub preview_with_chafa: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeRatePluginConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_true")]
    pub free_fallback_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageGenerationPluginConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_image_generation_provider_type")]
    pub provider_type: String,
    #[serde(default = "default_openai_images_base_url")]
    pub base_url: String,
    #[serde(default)]
    pub api_keys: Vec<String>,
    #[serde(default = "default_image_generation_model")]
    pub model: String,
    #[serde(default = "default_image_generation_aspect_ratio")]
    pub default_aspect_ratio: String,
    #[serde(default = "default_image_generation_resolution")]
    pub default_resolution: String,
    #[serde(default = "default_image_generation_output_dir")]
    pub output_dir: String,
    #[serde(default)]
    pub auto_print: bool,
    #[serde(default = "default_image_generation_timeout")]
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrintImagePluginConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_print_image_width_percent")]
    pub width_percent: u8,
    #[serde(default = "default_print_image_height_percent")]
    pub height_percent: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemesPluginConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 表情库映射；人格删除后只读 default 键，字段名保留以兼容旧配置
    #[serde(default)]
    pub libraries: HashMap<String, String>,
    #[serde(default = "default_memes_width_percent")]
    pub width_percent: u8,
    #[serde(default = "default_memes_height_percent")]
    pub height_percent: u8,
    #[serde(default = "default_memes_max_image_mb")]
    pub max_image_mb: u64,
    #[serde(default)]
    pub allow_gif_animation: bool,
    #[serde(default)]
    pub auto_send_enabled: bool,
    #[serde(default = "default_memes_auto_send_probability")]
    pub auto_send_probability: f32,
    #[serde(default = "default_memes_auto_send_min_confidence")]
    pub auto_send_min_confidence: f32,
}

impl MemesPluginConfig {

    /// 返回当前使用的表情库名称。
    ///
    /// 人格系统删除后只有一个库，映射表里除 default 之外的键不再被读取。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 配置的默认库名；未配置时为 sai
    pub fn default_library(&self) -> String {
        self.libraries
            .get("default")
            .cloned()
            .unwrap_or_else(|| "sai".to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeBasePluginConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub data_dir: String,
    #[serde(default = "default_kb_max_search_results")]
    pub max_search_results: usize,
    #[serde(default = "default_kb_snippet_context_chars")]
    pub snippet_context_chars: usize,
    #[serde(default = "default_kb_proximity_window_chars")]
    pub proximity_window_chars: usize,
    #[serde(default = "default_kb_max_read_lines")]
    pub max_read_lines: usize,
    #[serde(default = "default_kb_max_file_size_kb")]
    pub max_file_size_kb: usize,
    #[serde(default = "default_kb_allowed_extensions")]
    pub allowed_extensions: String,
    #[serde(default = "default_kb_allowed_filenames")]
    pub allowed_filenames: String,
    #[serde(default = "default_true")]
    pub upload_tool_enabled: bool,
    #[serde(default = "default_true")]
    pub embedding_enabled: bool,
    #[serde(default)]
    pub embedding_provider_id: String,
    #[serde(default)]
    pub embedding_model: String,
    #[serde(default = "default_kb_semantic_chunk_chars")]
    pub semantic_chunk_chars: usize,
    #[serde(default = "default_kb_semantic_chunk_overlap")]
    pub semantic_chunk_overlap: usize,
    #[serde(default = "default_kb_semantic_top_k")]
    pub semantic_top_k: usize,
    #[serde(default = "default_kb_semantic_min_score")]
    pub semantic_min_score: f32,
    #[serde(default = "default_kb_keyword_strong_score_threshold")]
    pub keyword_strong_score_threshold: f32,
    #[serde(default = "default_kb_embedding_timeout_seconds")]
    pub embedding_timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalculatorPluginConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_calculator_backend")]
    pub backend: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticsPluginConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_diagnostics_timeout")]
    pub command_timeout_seconds: u64,
    #[serde(default = "default_diagnostics_max_stdout_chars")]
    pub max_stdout_chars: usize,
    #[serde(default = "default_diagnostics_max_stderr_chars")]
    pub max_stderr_chars: usize,
}

impl PluginsConfig {
    /// 返回所有插件均启用的副本。
    ///
    /// 工具目录与白名单诊断都要回答"这个工具在系统里存不存在"，那与用户
    /// 当前开了哪些插件无关：关掉汇率插件不该让汇率工具从 Agent 配置界面
    /// 上消失，否则想启用它的人根本勾不到。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 全部插件开关置为 true 的副本
    pub fn all_enabled(&self) -> Self {
        let mut plugins = self.clone();
        plugins.weather.enabled = true;
        plugins.web.enabled = true;
        plugins.web_images.enabled = true;
        plugins.deep_diagnose.enabled = true;
        plugins.vision.enabled = true;
        plugins.exchange_rate.enabled = true;
        plugins.xuanxue.enabled = true;
        plugins.image_generation.enabled = true;
        plugins.print_image.enabled = true;
        plugins.memes.enabled = true;
        plugins.knowledge_base.enabled = true;
        plugins.archlinux.enabled = true;
        plugins.man.enabled = true;
        plugins.moegirl.enabled = true;
        plugins.hash_codec.enabled = true;
        plugins.calculator.enabled = true;
        plugins.package_advisor.enabled = true;
        plugins.linux_game_compatibility.enabled = true;
        plugins.diagnostics.enabled = true;
        plugins.memory.enabled = true;
        plugins
    }
}

#[cfg(test)]
mod all_enabled_tests {
    use super::*;

    /// 验证每一个插件开关都被打开。
    ///
    /// 逐字段赋值必然会在新增插件时漏掉，而漏掉的表现是那个工具在 Agent
    /// 配置界面上不可见——没有任何东西会报错。这里按序列化结果遍历，
    /// 新插件只要带 enabled 字段就会被这条测试抓住。
    #[test]
    fn every_plugin_switch_is_turned_on() {
        // 先关掉一批，确保通过不是因为默认值本来就是 true
        let mut plugins = PluginsConfig::default();
        plugins.weather.enabled = false;
        plugins.web.enabled = false;
        plugins.calculator.enabled = false;
        plugins.exchange_rate.enabled = false;
        plugins.hash_codec.enabled = false;

        let enabled = serde_json::to_value(plugins.all_enabled()).unwrap();

        for (name, value) in enabled.as_object().unwrap() {
            if let Some(flag) = value.get("enabled") {
                assert_eq!(flag, true, "插件 {name} 的开关没有被打开");
            }
        }
    }
}
