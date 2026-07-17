use crate::config::AppConfig;
use crate::i18n::text as t;
use anyhow::Result;

use super::form::{
    kb_embedding_provider_value, parse_bool_field, parse_provider_model_choice,
    provider_model_choice_values, vision_provider_value, Field,
};
use super::plugins::{plugin_enabled, toggle_plugin};

/// 构造指定插件的配置字段。
///
/// 参数:
/// - `config`: 当前应用配置
/// - `index`: 插件索引
///
/// 返回:
/// - 插件配置表单字段
pub(super) fn plugin_fields(config: &AppConfig, index: usize) -> Vec<Field> {
    match index {
        0 => vec![
            Field::boolean(t("Enabled", "启用"), config.plugins.web.enabled),
            Field::textarea(
                "TinyFish API Keys",
                config.plugins.web.tinyfish_api_keys.join("\n"),
            )
            .secret(),
            Field::textarea(
                "Tavily API Keys",
                config.plugins.web.tavily_api_keys.join("\n"),
            )
            .secret(),
            Field::textarea(
                "Firecrawl API Keys",
                config.plugins.web.firecrawl_api_keys.join("\n"),
            )
            .secret(),
            Field::textarea(
                "AnySearch API Keys",
                config.plugins.web.anysearch_api_keys.join("\n"),
            )
            .secret(),
            Field::new(
                t("SearXNG URL", "SearXNG 地址"),
                config.plugins.web.searxng_base_url.clone(),
            ),
        ],
        1 => vec![
            Field::boolean(t("Enabled", "启用"), config.plugins.deep_research.enabled),
            Field::new(
                t("Output directory", "输出目录"),
                config.plugins.deep_research.output_dir.clone(),
            ),
            Field::new(
                t("Thinking depth", "思考深度"),
                config.plugins.deep_research.thinking_depth.clone(),
            )
            .choices(&["minimal", "low", "medium", "high", "xhigh"]),
            Field::new(
                t("Max review revisions", "最大审视修正次数"),
                config
                    .plugins
                    .deep_research
                    .max_review_revisions
                    .to_string(),
            ),
            Field::new(
                t("Tool steps per round", "每轮工具步数"),
                config
                    .plugins
                    .deep_research
                    .max_tool_steps_per_round
                    .to_string(),
            ),
            Field::new(
                t("Final answer char limit", "最终字数上限"),
                config
                    .plugins
                    .deep_research
                    .max_final_answer_chars
                    .to_string(),
            ),
            Field::new(
                t("Tool timeout seconds", "工具超时秒数"),
                config
                    .plugins
                    .deep_research
                    .tool_call_timeout_seconds
                    .to_string(),
            ),
            Field::boolean(
                t("Show progress", "显示过程进度"),
                config.plugins.deep_research.show_progress,
            ),
        ],
        2 => vec![
            Field::boolean(t("Enabled", "启用"), config.plugins.vision.enabled),
            Field::boolean(
                t("Prefer current multimodal model", "优先当前多模态模型"),
                config.plugins.vision.prefer_current_multimodal_model,
            ),
            Field::new(
                t("Vision Provider/model", "识图 Provider/模型"),
                vision_provider_value(config),
            )
            .choices_owned(provider_model_choice_values(config, true)),
            Field::boolean(
                t("Preview with chafa", "使用 chafa 预览"),
                config.plugins.vision.preview_with_chafa,
            ),
        ],
        3 => vec![
            Field::boolean(
                t("Enabled", "启用"),
                config.plugins.image_generation.enabled,
            ),
            Field::new(
                t("Image API type", "生图 API 类型"),
                config.plugins.image_generation.provider_type.clone(),
            )
            .choices(&["openai", "rightcode"]),
            Field::new(
                t("Base URL", "基础地址"),
                config.plugins.image_generation.base_url.clone(),
            ),
            Field::textarea(
                "API Keys",
                config.plugins.image_generation.api_keys.join("\n"),
            )
            .secret(),
            Field::new(
                t("Model", "模型"),
                config.plugins.image_generation.model.clone(),
            ),
            Field::new(
                t("Default aspect ratio", "默认宽高比"),
                config.plugins.image_generation.default_aspect_ratio.clone(),
            )
            .choices(&[
                "自动", "1:1", "2:3", "3:2", "3:4", "4:3", "4:5", "5:4", "9:16", "16:9", "21:9",
            ]),
            Field::new(
                t("Default resolution", "默认分辨率"),
                config.plugins.image_generation.default_resolution.clone(),
            )
            .choices(&["1K", "2K", "4K"]),
            Field::new(
                t("Output directory", "输出目录"),
                config.plugins.image_generation.output_dir.clone(),
            ),
            Field::boolean(
                t("Print after completion", "完成后打印"),
                config.plugins.image_generation.auto_print,
            ),
            Field::new(
                t("Timeout seconds", "超时秒数"),
                config.plugins.image_generation.timeout_seconds.to_string(),
            ),
        ],
        4 => vec![
            Field::boolean(t("Enabled", "启用"), config.plugins.web_images.enabled),
            Field::boolean(
                t("Vision model screening", "视觉模型审核"),
                config.plugins.web_images.vision_screening_enabled,
            ),
            Field::new(
                t("Max results", "数量上限"),
                config.plugins.web_images.max_results.to_string(),
            ),
            Field::boolean(
                t("Safe search", "安全搜索"),
                config.plugins.web_images.safe_search,
            ),
            Field::boolean(
                t("Auto preview", "自动预览"),
                config.plugins.web_images.auto_preview,
            ),
            Field::new(
                t("Default preview count", "默认预览数量"),
                config.plugins.web_images.preview_count.to_string(),
            ),
            Field::new(
                t("Max download MB", "最大下载 MB"),
                config.plugins.web_images.max_download_mb.to_string(),
            ),
            Field::new(
                t("Timeout seconds", "超时秒数"),
                config.plugins.web_images.timeout_seconds.to_string(),
            ),
        ],
        5 => vec![
            Field::boolean(t("Enabled", "启用"), config.plugins.print_image.enabled),
            Field::new(
                t("Print width percent", "打印宽度百分比"),
                config.plugins.print_image.width_percent.to_string(),
            ),
            Field::new(
                t("Print height percent", "打印高度百分比"),
                config.plugins.print_image.height_percent.to_string(),
            ),
        ],
        6 => vec![
            Field::boolean(t("Enabled", "启用"), config.plugins.memes.enabled),
            Field::new(
                t("Send width percent", "发送宽度百分比"),
                config.plugins.memes.width_percent.to_string(),
            ),
            Field::new(
                t("Send height percent", "发送高度百分比"),
                config.plugins.memes.height_percent.to_string(),
            ),
            Field::new(
                t("Max image MB", "最大图片 MB"),
                config.plugins.memes.max_image_mb.to_string(),
            ),
            Field::boolean(
                t("Allow GIF animation", "允许 GIF 动画"),
                config.plugins.memes.allow_gif_animation,
            ),
            Field::boolean(
                t("Auto send", "自动发送"),
                config.plugins.memes.auto_send_enabled,
            ),
            Field::new(
                t("Auto send probability", "自动发送概率"),
                config.plugins.memes.auto_send_probability.to_string(),
            ),
            Field::new(
                t("Auto send minimum confidence", "自动发送最低置信度"),
                config.plugins.memes.auto_send_min_confidence.to_string(),
            ),
        ],
        7 => vec![
            Field::boolean(t("Enabled", "启用"), config.plugins.knowledge_base.enabled),
            Field::new(
                t("Knowledge base directory", "知识库目录"),
                config.plugins.knowledge_base.data_dir.clone(),
            ),
            Field::new(
                t("Max search results", "搜索最大结果数"),
                config.plugins.knowledge_base.max_search_results.to_string(),
            ),
            Field::new(
                t("Snippet context chars", "片段上下文字数"),
                config
                    .plugins
                    .knowledge_base
                    .snippet_context_chars
                    .to_string(),
            ),
            Field::new(
                t("Proximity window chars", "同窗匹配范围"),
                config
                    .plugins
                    .knowledge_base
                    .proximity_window_chars
                    .to_string(),
            ),
            Field::new(
                t("Max read lines", "读取最大行数"),
                config.plugins.knowledge_base.max_read_lines.to_string(),
            ),
            Field::new(
                t("Max file KB", "最大文件 KB"),
                config.plugins.knowledge_base.max_file_size_kb.to_string(),
            ),
            Field::new(
                t("Allowed extensions", "允许扩展名"),
                config.plugins.knowledge_base.allowed_extensions.clone(),
            ),
            Field::new(
                t("Allowed filenames", "允许文件名"),
                config.plugins.knowledge_base.allowed_filenames.clone(),
            ),
            Field::boolean(
                t("Allow AI upload", "允许 AI 上传"),
                config.plugins.knowledge_base.upload_tool_enabled,
            ),
            Field::boolean(
                t("Embedding enabled", "启用 Embedding"),
                config.plugins.knowledge_base.embedding_enabled,
            ),
            Field::new(
                t("Embedding Provider/model", "Embedding Provider/模型"),
                kb_embedding_provider_value(config),
            )
            .choices_owned(provider_model_choice_values(config, false))
            .empty_choice_label(t("Embedding not configured", "未配置 Embedding")),
            Field::new(
                t("Semantic chunk chars", "语义块大小"),
                config
                    .plugins
                    .knowledge_base
                    .semantic_chunk_chars
                    .to_string(),
            ),
            Field::new(
                t("Semantic chunk overlap", "语义块重叠"),
                config
                    .plugins
                    .knowledge_base
                    .semantic_chunk_overlap
                    .to_string(),
            ),
            Field::new(
                t("Semantic top K", "语义候选数"),
                config.plugins.knowledge_base.semantic_top_k.to_string(),
            ),
            Field::new(
                t("Semantic minimum score", "语义最低分"),
                config.plugins.knowledge_base.semantic_min_score.to_string(),
            ),
            Field::new(
                t("Keyword strong score threshold", "关键词强命中阈值"),
                config
                    .plugins
                    .knowledge_base
                    .keyword_strong_score_threshold
                    .to_string(),
            ),
            Field::new(
                t("Embedding timeout seconds", "Embedding 超时秒数"),
                config
                    .plugins
                    .knowledge_base
                    .embedding_timeout_seconds
                    .to_string(),
            ),
        ],
        8 => vec![Field::boolean(
            t("Enabled", "启用"),
            config.plugins.archlinux.enabled,
        )],
        9 => vec![Field::boolean(
            t("Enabled", "启用"),
            config.plugins.man.enabled,
        )],
        10 => vec![
            Field::boolean(t("Enabled", "启用"), config.plugins.memory.enabled),
            Field::boolean(
                t("Evicted context cache", "上下文弹出缓存"),
                config.plugins.memory.evicted_context_enabled,
            ),
            Field::boolean(
                t("Association enabled", "联想启用"),
                config.plugins.memory.association_enabled,
            ),
            Field::boolean(
                t("Auto diary", "自动日记"),
                config.plugins.memory.auto_diary_enabled,
            ),
            Field::boolean(
                t("Auto fact memory", "自动知识记忆"),
                config.plugins.memory.auto_fact_enabled,
            ),
            Field::boolean(
                t("Auto skill memory", "自动技能记忆"),
                config.plugins.memory.auto_skill_enabled,
            ),
            Field::new(
                t("Association facts", "联想知识条数"),
                config.plugins.memory.association_facts.to_string(),
            ),
            Field::new(
                t("Association episodes", "联想事件条数"),
                config.plugins.memory.association_episodes.to_string(),
            ),
            Field::new(
                t("Association char limit", "联想字符上限"),
                config.plugins.memory.association_max_chars.to_string(),
            ),
            Field::new(
                t("Memory snippet chars", "记忆片段字符数"),
                config.plugins.memory.snippet_chars.to_string(),
            ),
            Field::new(
                t("Forget after days", "记忆保留天数"),
                config.plugins.memory.forget_after_days.to_string(),
            ),
            Field::boolean(
                t("Forgetting enabled", "遗忘启用"),
                config.plugins.memory.forgetting_enabled,
            ),
            Field::new(
                t("Forgetting half-life days", "遗忘半衰期天"),
                config.plugins.memory.forgetting_half_life_days.to_string(),
            ),
            Field::new(
                t("Forgetting minimum strength", "遗忘最低强度"),
                config.plugins.memory.forgetting_min_strength.to_string(),
            ),
            Field::new(
                t("Recall review boost", "回忆增强强度"),
                config.plugins.memory.forgetting_review_boost.to_string(),
            ),
            Field::new(
                t("Learning minimum task chars", "学习任务最少字符数"),
                config.plugins.memory.learning_min_task_chars.to_string(),
            ),
            Field::new(
                t("Learning minimum method chars", "学习方法最少字符数"),
                config.plugins.memory.learning_min_method_chars.to_string(),
            ),
        ],
        11 => vec![Field::boolean(
            t("Enabled", "启用"),
            config.plugins.package_advisor.enabled,
        )],
        12 => vec![
            Field::boolean(
                t("Enabled", "启用"),
                config.plugins.linux_game_compatibility.enabled,
            ),
            Field::new(
                t("Subagent max tool steps", "子代理最大工具次数"),
                config
                    .plugins
                    .linux_game_compatibility
                    .max_tool_steps
                    .to_string(),
            ),
        ],
        13 => vec![
            Field::boolean(t("Enabled", "启用"), config.plugins.deep_diagnose.enabled),
            Field::new(
                t("Thinking depth", "思考深度"),
                config.plugins.deep_diagnose.thinking_depth.clone(),
            )
            .choices(&["minimal", "low", "medium", "high", "xhigh"]),
            Field::new(
                t("Max review revisions", "最大审视修正次数"),
                config
                    .plugins
                    .deep_diagnose
                    .max_review_revisions
                    .to_string(),
            ),
            Field::new(
                t("Tool steps per round", "每轮工具步数"),
                config
                    .plugins
                    .deep_diagnose
                    .max_tool_steps_per_round
                    .to_string(),
            ),
            Field::new(
                t("Final answer char limit", "最终字数上限"),
                config
                    .plugins
                    .deep_diagnose
                    .max_final_answer_chars
                    .to_string(),
            ),
            Field::new(
                t("Tool timeout seconds", "工具超时秒数"),
                config
                    .plugins
                    .deep_diagnose
                    .tool_call_timeout_seconds
                    .to_string(),
            ),
            Field::new(
                t("Maximum tool steps", "最大工具次数"),
                config.plugins.deep_diagnose.max_tool_steps.to_string(),
            ),
            Field::boolean(
                t("Show progress", "显示过程进度"),
                config.plugins.deep_diagnose.show_progress,
            ),
        ],
        14 => vec![
            Field::boolean(t("Enabled", "启用"), config.plugins.diagnostics.enabled),
            Field::new(
                t("Command timeout seconds", "命令超时秒数"),
                config
                    .plugins
                    .diagnostics
                    .command_timeout_seconds
                    .to_string(),
            ),
            Field::new(
                t("Maximum stdout chars", "标准输出字符上限"),
                config.plugins.diagnostics.max_stdout_chars.to_string(),
            ),
            Field::new(
                t("Maximum stderr chars", "错误输出字符上限"),
                config.plugins.diagnostics.max_stderr_chars.to_string(),
            ),
        ],
        _ => vec![Field::boolean(
            t("Enabled", "启用"),
            plugin_enabled(config, index),
        )],
    }
}

/// 将插件表单字段写回配置。
///
/// 参数:
/// - `config`: 当前应用配置
/// - `index`: 插件索引
/// - `fields`: 表单字段
///
/// 返回:
/// - 写回是否成功
pub(super) fn apply_plugin_fields(
    config: &mut AppConfig,
    index: usize,
    fields: &[Field],
) -> Result<()> {
    match index {
        0 => {
            config.plugins.web.enabled = parse_bool_field(&fields[0].value)?;
            config.plugins.web.tinyfish_api_keys = parse_key_list(&fields[1].value);
            config.plugins.web.tavily_api_keys = parse_key_list(&fields[2].value);
            config.plugins.web.firecrawl_api_keys = parse_key_list(&fields[3].value);
            config.plugins.web.anysearch_api_keys = parse_key_list(&fields[4].value);
            config.plugins.web.searxng_base_url =
                fields[5].value.trim().trim_end_matches('/').to_string();
        }
        1 => {
            config.plugins.deep_research.enabled = parse_bool_field(&fields[0].value)?;
            config.plugins.deep_research.output_dir = fields[1].value.trim().to_string();
            config.plugins.deep_research.thinking_depth = fields[2].value.trim().to_string();
            config.plugins.deep_research.max_review_revisions = fields[3].value.trim().parse()?;
            config.plugins.deep_research.max_tool_steps_per_round =
                fields[4].value.trim().parse()?;
            config.plugins.deep_research.max_final_answer_chars = fields[5].value.trim().parse()?;
            config.plugins.deep_research.tool_call_timeout_seconds =
                fields[6].value.trim().parse()?;
            config.plugins.deep_research.show_progress = parse_bool_field(&fields[7].value)?;
        }
        2 => {
            config.plugins.vision.enabled = parse_bool_field(&fields[0].value)?;
            config.plugins.vision.prefer_current_multimodal_model =
                parse_bool_field(&fields[1].value)?;
            let (provider_id, model) = parse_provider_model_choice(&fields[2].value);
            config.plugins.vision.vision_provider_id = provider_id;
            config.plugins.vision.vision_model = model;
            config.plugins.vision.preview_with_chafa = parse_bool_field(&fields[3].value)?;
        }
        3 => {
            config.plugins.image_generation.enabled = parse_bool_field(&fields[0].value)?;
            config.plugins.image_generation.provider_type = fields[1].value.trim().to_string();
            config.plugins.image_generation.base_url =
                fields[2].value.trim().trim_end_matches('/').to_string();
            config.plugins.image_generation.api_keys = parse_key_list(&fields[3].value);
            config.plugins.image_generation.model = fields[4].value.trim().to_string();
            config.plugins.image_generation.default_aspect_ratio =
                fields[5].value.trim().to_string();
            config.plugins.image_generation.default_resolution = fields[6].value.trim().to_string();
            config.plugins.image_generation.output_dir = fields[7].value.trim().to_string();
            config.plugins.image_generation.auto_print = parse_bool_field(&fields[8].value)?;
            config.plugins.image_generation.timeout_seconds = fields[9].value.trim().parse()?;
        }
        4 => {
            config.plugins.web_images.enabled = parse_bool_field(&fields[0].value)?;
            config.plugins.web_images.vision_screening_enabled =
                parse_bool_field(&fields[1].value)?;
            config.plugins.web_images.max_results =
                fields[2].value.trim().parse::<usize>()?.clamp(1, 10);
            config.plugins.web_images.safe_search = parse_bool_field(&fields[3].value)?;
            config.plugins.web_images.auto_preview = parse_bool_field(&fields[4].value)?;
            config.plugins.web_images.preview_count =
                fields[5].value.trim().parse::<usize>()?.min(5);
            config.plugins.web_images.max_download_mb =
                fields[6].value.trim().parse::<f64>()?.clamp(0.1, 50.0);
            config.plugins.web_images.timeout_seconds =
                fields[7].value.trim().parse::<u64>()?.clamp(5, 120);
        }
        5 => {
            config.plugins.print_image.enabled = parse_bool_field(&fields[0].value)?;
            config.plugins.print_image.width_percent = fields[1].value.trim().parse::<u8>()?;
            config.plugins.print_image.height_percent = fields[2].value.trim().parse::<u8>()?;
        }
        6 => {
            config.plugins.memes.enabled = parse_bool_field(&fields[0].value)?;
            config.plugins.memes.width_percent =
                fields[1].value.trim().parse::<u8>()?.clamp(1, 100);
            config.plugins.memes.height_percent =
                fields[2].value.trim().parse::<u8>()?.clamp(1, 100);
            config.plugins.memes.max_image_mb =
                fields[3].value.trim().parse::<u64>()?.clamp(1, 100);
            config.plugins.memes.allow_gif_animation = parse_bool_field(&fields[4].value)?;
            config.plugins.memes.auto_send_enabled = parse_bool_field(&fields[5].value)?;
            config.plugins.memes.auto_send_probability =
                fields[6].value.trim().parse::<f32>()?.clamp(0.0, 1.0);
            config.plugins.memes.auto_send_min_confidence =
                fields[7].value.trim().parse::<f32>()?.clamp(0.0, 1.0);
        }
        7 => {
            config.plugins.knowledge_base.enabled = parse_bool_field(&fields[0].value)?;
            config.plugins.knowledge_base.data_dir = fields[1].value.trim().to_string();
            config.plugins.knowledge_base.max_search_results = fields[2].value.trim().parse()?;
            config.plugins.knowledge_base.snippet_context_chars = fields[3].value.trim().parse()?;
            config.plugins.knowledge_base.proximity_window_chars =
                fields[4].value.trim().parse()?;
            config.plugins.knowledge_base.max_read_lines = fields[5].value.trim().parse()?;
            config.plugins.knowledge_base.max_file_size_kb = fields[6].value.trim().parse()?;
            config.plugins.knowledge_base.allowed_extensions = fields[7].value.trim().to_string();
            config.plugins.knowledge_base.allowed_filenames = fields[8].value.trim().to_string();
            config.plugins.knowledge_base.upload_tool_enabled = parse_bool_field(&fields[9].value)?;
            config.plugins.knowledge_base.embedding_enabled = parse_bool_field(&fields[10].value)?;
            let (provider_id, model) = parse_provider_model_choice(&fields[11].value);
            config.plugins.knowledge_base.embedding_provider_id = provider_id;
            config.plugins.knowledge_base.embedding_model = model;
            config.plugins.knowledge_base.semantic_chunk_chars = fields[12].value.trim().parse()?;
            config.plugins.knowledge_base.semantic_chunk_overlap =
                fields[13].value.trim().parse()?;
            config.plugins.knowledge_base.semantic_top_k = fields[14].value.trim().parse()?;
            config.plugins.knowledge_base.semantic_min_score = fields[15].value.trim().parse()?;
            config.plugins.knowledge_base.keyword_strong_score_threshold =
                fields[16].value.trim().parse()?;
            config.plugins.knowledge_base.embedding_timeout_seconds =
                fields[17].value.trim().parse()?;
        }
        8 => {
            config.plugins.archlinux.enabled = parse_bool_field(&fields[0].value)?;
        }
        9 => {
            config.plugins.man.enabled = parse_bool_field(&fields[0].value)?;
        }
        10 => {
            config.plugins.memory.enabled = parse_bool_field(&fields[0].value)?;
            config.plugins.memory.evicted_context_enabled = parse_bool_field(&fields[1].value)?;
            config.plugins.memory.association_enabled = parse_bool_field(&fields[2].value)?;
            config.plugins.memory.auto_diary_enabled = parse_bool_field(&fields[3].value)?;
            config.plugins.memory.auto_fact_enabled = parse_bool_field(&fields[4].value)?;
            config.plugins.memory.auto_skill_enabled = parse_bool_field(&fields[5].value)?;
            config.plugins.memory.association_facts = fields[6].value.trim().parse::<usize>()?;
            config.plugins.memory.association_episodes = fields[7].value.trim().parse::<usize>()?;
            config.plugins.memory.association_max_chars =
                fields[8].value.trim().parse::<usize>()?;
            config.plugins.memory.snippet_chars = fields[9].value.trim().parse::<usize>()?;
            config.plugins.memory.forget_after_days = fields[10].value.trim().parse::<u64>()?;
            config.plugins.memory.forgetting_enabled = parse_bool_field(&fields[11].value)?;
            config.plugins.memory.forgetting_half_life_days =
                fields[12].value.trim().parse::<f64>()?;
            config.plugins.memory.forgetting_min_strength =
                fields[13].value.trim().parse::<f64>()?;
            config.plugins.memory.forgetting_review_boost =
                fields[14].value.trim().parse::<f64>()?;
            config.plugins.memory.learning_min_task_chars =
                fields[15].value.trim().parse::<usize>()?;
            config.plugins.memory.learning_min_method_chars =
                fields[16].value.trim().parse::<usize>()?;
        }
        11 => {
            config.plugins.package_advisor.enabled = parse_bool_field(&fields[0].value)?;
        }
        12 => {
            config.plugins.linux_game_compatibility.enabled = parse_bool_field(&fields[0].value)?;
            config.plugins.linux_game_compatibility.max_tool_steps =
                fields[1].value.trim().parse::<usize>()?.clamp(1, 500);
        }
        13 => {
            config.plugins.deep_diagnose.enabled = parse_bool_field(&fields[0].value)?;
            config.plugins.deep_diagnose.thinking_depth = fields[1].value.trim().to_string();
            config.plugins.deep_diagnose.max_review_revisions = fields[2].value.trim().parse()?;
            config.plugins.deep_diagnose.max_tool_steps_per_round =
                fields[3].value.trim().parse()?;
            config.plugins.deep_diagnose.max_final_answer_chars = fields[4].value.trim().parse()?;
            config.plugins.deep_diagnose.tool_call_timeout_seconds =
                fields[5].value.trim().parse()?;
            config.plugins.deep_diagnose.max_tool_steps = fields[6].value.trim().parse()?;
            config.plugins.deep_diagnose.show_progress = parse_bool_field(&fields[7].value)?;
        }
        14 => {
            config.plugins.diagnostics.enabled = parse_bool_field(&fields[0].value)?;
            config.plugins.diagnostics.command_timeout_seconds = fields[1].value.trim().parse()?;
            config.plugins.diagnostics.max_stdout_chars = fields[2].value.trim().parse()?;
            config.plugins.diagnostics.max_stderr_chars = fields[3].value.trim().parse()?;
        }
        _ => {
            let value = parse_bool_field(&fields[0].value)?;
            if plugin_enabled(config, index) != value {
                toggle_plugin(config, index);
            }
        }
    }
    Ok(())
}

/// 解析多行或逗号分隔的 API Key 列表。
///
/// 参数:
/// - `value`: 表单输入
///
/// 返回:
/// - 去除空白后的 Key 列表
fn parse_key_list(value: &str) -> Vec<String> {
    value
        .split([',', '\n', '\r'])
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
#[path = "plugin_fields_tests.rs"]
mod tests;
