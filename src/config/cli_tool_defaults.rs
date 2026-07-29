use super::cli_tools::*;
use super::defaults::*;
use super::model::MemoryConfig;
use std::collections::HashMap;

impl Default for PluginsConfig {
    /// 构造全部 CLI 助手工具与 Web 搜索的默认配置。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 历史 `plugins` 键对应的完整默认配置
    fn default() -> Self {
        Self {
            weather: PluginEnabledConfig::default(),
            web: WebSearchConfig::default(),
            web_images: WebImagesPluginConfig::default(),
            deep_research: DeepResearchPluginConfig::default(),
            deep_diagnose: DeepDiagnosePluginConfig::default(),
            vision: VisionPluginConfig::default(),
            exchange_rate: ExchangeRatePluginConfig::default(),
            xuanxue: PluginEnabledConfig::default(),
            image_generation: ImageGenerationPluginConfig::default(),
            print_image: PrintImagePluginConfig::default(),
            memes: MemesPluginConfig::default(),
            knowledge_base: KnowledgeBasePluginConfig::default(),
            archlinux: PluginEnabledConfig::default(),
            man: PluginEnabledConfig::default(),
            moegirl: PluginEnabledConfig::default(),
            hash_codec: PluginEnabledConfig::default(),
            calculator: CalculatorPluginConfig::default(),
            package_advisor: PluginEnabledConfig::default(),
            linux_game_compatibility: LinuxGameCompatibilityConfig::default(),
            diagnostics: DiagnosticsPluginConfig::default(),
            memory: MemoryConfig::default(),
        }
    }
}

impl Default for PluginEnabledConfig {
    /// 构造默认启用的简单工具配置。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 默认启用状态
    fn default() -> Self {
        Self {
            enabled: default_true(),
        }
    }
}

impl Default for LinuxGameCompatibilityConfig {
    /// 构造 Linux 游戏兼容性工具默认配置。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 默认启用状态与子任务步数上限
    fn default() -> Self {
        Self {
            enabled: default_true(),
            max_tool_steps: default_subagent_max_tool_steps(),
        }
    }
}

impl Default for WebSearchConfig {
    /// 构造 Web 搜索与各供应商的默认配置。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 自动路由和各供应商默认参数
    fn default() -> Self {
        Self {
            enabled: default_true(),
            default_provider: default_web_search_provider(),
            max_results: default_web_search_max_results(),
            timeout_seconds: default_web_search_timeout(),
            tinyfish_enabled: default_true(),
            tinyfish_api_keys: Vec::new(),
            tinyfish_base_url: default_tinyfish_base_url(),
            tinyfish_default_location: String::new(),
            tinyfish_default_language: String::new(),
            tavily_enabled: default_true(),
            tavily_api_keys: Vec::new(),
            tavily_base_url: default_tavily_base_url(),
            tavily_search_depth: default_tavily_search_depth(),
            tavily_include_answer: false,
            tavily_include_raw_content: default_true(),
            firecrawl_enabled: default_true(),
            firecrawl_api_keys: Vec::new(),
            firecrawl_base_url: default_firecrawl_base_url(),
            firecrawl_only_main_content: default_true(),
            anysearch_enabled: default_true(),
            anysearch_api_keys: Vec::new(),
            anysearch_base_url: default_anysearch_base_url(),
            searxng_enabled: default_true(),
            searxng_base_url: String::new(),
            searxng_language: default_searxng_language(),
            searxng_safe_search: 0,
            duckduckgo_enabled: default_true(),
        }
    }
}

impl Default for WebImagesPluginConfig {
    /// 构造网页图片工具默认配置。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 搜图数量、下载限制和预览策略
    fn default() -> Self {
        Self {
            enabled: default_true(),
            max_results: default_web_images_max_results(),
            max_download_mb: default_web_images_max_download_mb(),
            safe_search: default_true(),
            vision_screening_enabled: default_true(),
            auto_preview: default_true(),
            preview_count: default_web_images_preview_count(),
            timeout_seconds: default_web_images_timeout(),
        }
    }
}

impl Default for DeepResearchPluginConfig {
    /// 构造深度研究工具默认配置。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 默认研究深度、审阅轮数和超时参数
    fn default() -> Self {
        Self {
            enabled: default_true(),
            output_dir: default_deep_research_dir(),
            thinking_depth: default_deep_research_depth(),
            max_review_revisions: default_deep_research_max_review_revisions(),
            max_tool_steps_per_round: default_deep_research_max_tool_steps(),
            max_final_answer_chars: 0,
            tool_call_timeout_seconds: default_deep_research_tool_timeout(),
            show_progress: default_true(),
        }
    }
}

impl Default for DeepDiagnosePluginConfig {
    /// 构造深度诊断工具默认配置。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 默认诊断深度、审阅轮数和工具步数上限
    fn default() -> Self {
        Self {
            enabled: default_true(),
            thinking_depth: default_deep_research_depth(),
            max_review_revisions: default_deep_research_max_review_revisions(),
            max_tool_steps_per_round: default_deep_research_max_tool_steps(),
            max_final_answer_chars: 0,
            tool_call_timeout_seconds: default_deep_research_tool_timeout(),
            max_tool_steps: default_subagent_max_tool_steps(),
            show_progress: default_true(),
        }
    }
}

impl Default for VisionPluginConfig {
    /// 构造视觉理解工具默认配置。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 当前模型优先的默认视觉配置
    fn default() -> Self {
        Self {
            enabled: default_true(),
            prefer_current_multimodal_model: default_true(),
            vision_provider_id: String::new(),
            vision_model: String::new(),
            preview_with_chafa: default_true(),
        }
    }
}

impl Default for ExchangeRatePluginConfig {
    /// 构造汇率查询工具默认配置。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 默认启用且允许免费回退的配置
    fn default() -> Self {
        Self {
            enabled: default_true(),
            api_key: String::new(),
            free_fallback_enabled: default_true(),
        }
    }
}

impl Default for ImageGenerationPluginConfig {
    /// 构造图片生成工具默认配置。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 默认关闭的图片供应商、模型与输出参数
    fn default() -> Self {
        Self {
            enabled: false,
            provider_type: default_image_generation_provider_type(),
            base_url: default_openai_images_base_url(),
            api_keys: Vec::new(),
            model: default_image_generation_model(),
            default_aspect_ratio: default_image_generation_aspect_ratio(),
            default_resolution: default_image_generation_resolution(),
            output_dir: default_image_generation_output_dir(),
            auto_print: default_true(),
            timeout_seconds: default_image_generation_timeout(),
        }
    }
}

impl Default for PrintImagePluginConfig {
    /// 构造终端图片输出工具默认配置。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 默认启用状态与终端占用比例
    fn default() -> Self {
        Self {
            enabled: default_true(),
            width_percent: default_print_image_width_percent(),
            height_percent: default_print_image_height_percent(),
        }
    }
}

impl Default for MemesPluginConfig {
    /// 构造表情图库工具默认配置。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 默认图库映射、显示尺寸和自动发送参数
    fn default() -> Self {
        Self {
            enabled: default_true(),
            persona_libraries: HashMap::new(),
            width_percent: default_memes_width_percent(),
            height_percent: default_memes_height_percent(),
            max_image_mb: default_memes_max_image_mb(),
            allow_gif_animation: false,
            auto_send_enabled: true,
            auto_send_probability: default_memes_auto_send_probability(),
            auto_send_min_confidence: default_memes_auto_send_min_confidence(),
        }
    }
}

impl Default for KnowledgeBasePluginConfig {
    /// 构造知识库工具默认配置。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 文件检索、语义索引和超时默认参数
    fn default() -> Self {
        Self {
            enabled: default_true(),
            data_dir: String::new(),
            max_search_results: default_kb_max_search_results(),
            snippet_context_chars: default_kb_snippet_context_chars(),
            proximity_window_chars: default_kb_proximity_window_chars(),
            max_read_lines: default_kb_max_read_lines(),
            max_file_size_kb: default_kb_max_file_size_kb(),
            allowed_extensions: default_kb_allowed_extensions(),
            allowed_filenames: default_kb_allowed_filenames(),
            upload_tool_enabled: default_true(),
            embedding_enabled: false,
            embedding_provider_id: String::new(),
            embedding_model: String::new(),
            semantic_chunk_chars: default_kb_semantic_chunk_chars(),
            semantic_chunk_overlap: default_kb_semantic_chunk_overlap(),
            semantic_top_k: default_kb_semantic_top_k(),
            semantic_min_score: default_kb_semantic_min_score(),
            keyword_strong_score_threshold: default_kb_keyword_strong_score_threshold(),
            embedding_timeout_seconds: default_kb_embedding_timeout_seconds(),
        }
    }
}

impl Default for CalculatorPluginConfig {
    /// 构造计算器工具默认配置。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 默认启用且使用内置后端的配置
    fn default() -> Self {
        Self {
            enabled: default_true(),
            backend: default_calculator_backend(),
        }
    }
}

impl Default for DiagnosticsPluginConfig {
    /// 构造运行诊断工具默认配置。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 命令超时与输出大小默认限制
    fn default() -> Self {
        Self {
            enabled: default_true(),
            command_timeout_seconds: default_diagnostics_timeout(),
            max_stdout_chars: default_diagnostics_max_stdout_chars(),
            max_stderr_chars: default_diagnostics_max_stderr_chars(),
        }
    }
}
