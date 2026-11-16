//! 首次引导界面的视觉标签、卡片摘要与流程精简辅助。

use super::form::Field;
use super::theme::{help_line, pill, ACCENT, BRAND, DIM, OK, PILL_BG, RESET, WARN};
use crate::config::{ModelMetadata, ProviderConfig};
use crate::i18n::text as t;
use crate::paths::SaiPaths;

/// 【首次配置】【就绪状态】标识模板当前是否具备零表单直接启动条件。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum QuickStartKind {
    BuiltInFree,
    EnvReady,
    NeedsInput,
}

/// 【首次配置】【步骤分类】判断供应商是否无需手动录入凭据即可快速启用。
///
/// 参数:
/// - `provider`: 候选供应商配置
/// - `paths`: 应用路径（用于解析 secrets 或环境变量）
///
/// 返回:
/// - 就绪分类枚举
pub(super) fn quick_start_kind(provider: &ProviderConfig, paths: &SaiPaths) -> QuickStartKind {
    if provider.default_model.trim().is_empty() {
        return QuickStartKind::NeedsInput;
    }
    if provider.is_opencode_zen() {
        return QuickStartKind::BuiltInFree;
    }
    if provider
        .resolved_api_key(paths)
        .is_ok_and(|key| !key.trim().is_empty())
    {
        return QuickStartKind::EnvReady;
    }
    QuickStartKind::NeedsInput
}

/// 【首次配置】【顶栏摘要】生成带步骤胶囊与状态提示的引导副标题。
///
/// 参数:
/// - `step`: 当前步骤序号（从 1 开始）
/// - `total`: 总步骤数
/// - `stage`: 当前阶段标题
/// - `notice`: 可选状态徽标文字
/// - `ok_notice`: 状态徽标是否使用成功色；false 时使用提示色
///
/// 返回:
/// - 已带 ANSI 样式的副标题文本
pub(super) fn step_subtitle(
    step: usize,
    total: usize,
    stage: &str,
    notice: Option<&str>,
    ok_notice: bool,
) -> String {
    let step_label = format!("{} {step}/{total}", t("Step", "步骤"));
    let mut parts = vec![format!(
        "{} {BRAND}◆{RESET} {ACCENT}{stage}{RESET}",
        pill(&step_label, BRAND, PILL_BG)
    )];
    if let Some(note) = notice.filter(|text| !text.trim().is_empty()) {
        let color = if ok_notice { OK } else { WARN };
        parts.push(pill(note, color, PILL_BG));
    }
    parts.join(&format!(" {DIM}·{RESET} "))
}

/// 【首次配置】【供应商卡片】构造左侧选项标签与右侧详情说明卡片。
///
/// 参数:
/// - `provider`: 候选供应商配置
/// - `kind`: 就绪状态
///
/// 返回:
/// - `(左侧显示名称, 右侧详情文本)`
pub(super) fn provider_menu_entry(
    provider: &ProviderConfig,
    kind: &QuickStartKind,
) -> (String, String) {
    let label = match kind {
        QuickStartKind::BuiltInFree => {
            format!("◆ {}  [{}]", provider.display_name, t("free", "免费"))
        }
        QuickStartKind::EnvReady => {
            format!("✓ {}  [{}]", provider.display_name, t("ready", "就绪"))
        }
        QuickStartKind::NeedsInput => format!("  {}", provider.display_name),
    };
    let status = match kind {
        QuickStartKind::BuiltInFree => t(
            "Ready now · built-in free tier, no API key required",
            "开箱即用 · 内置免费模型，无需填写密钥",
        ),
        QuickStartKind::EnvReady => t(
            "Credential detected · ready for instant start",
            "已检测到可用凭据 · 支持一键启动",
        ),
        QuickStartKind::NeedsInput if is_local_endpoint(&provider.base_url) => t(
            "Local endpoint · enter a model or any token if auth is off",
            "本地服务 · 若未开启鉴权可填写任意占位密钥",
        ),
        QuickStartKind::NeedsInput => t(
            "Setup needed · enter API key and choose model",
            "需要配置 · 填写接口密钥并选择默认模型",
        ),
    };
    let model_line = if provider.default_model.trim().is_empty() {
        t("Pick after connection check", "连接后从目录选择或填写").to_string()
    } else {
        provider.default_model.clone()
    };
    let actions = match kind {
        QuickStartKind::BuiltInFree => t(
            "Enter / Double-click: Start immediately\nm: Choose model   e: Edit endpoint",
            "Enter / 双击: 立即开始对话\nm: 选择模型   e: 编辑接入信息",
        ),
        QuickStartKind::EnvReady => t(
            "Space / s: Quick start with default model\nEnter / Double-click: Edit key or pick model",
            "Space / s: 使用默认模型一键启动\nEnter / 双击: 修改凭据或挑选模型",
        ),
        QuickStartKind::NeedsInput => t(
            "Enter / Double-click: Enter key & pick model",
            "Enter / 双击: 填写密钥并选择模型",
        ),
    };
    let detail = format!(
        "{status}\n\n{}: {model_line}\n{}: {} ({})\n\n{actions}",
        t("Default", "默认模型"),
        t("Endpoint", "接口地址"),
        provider.base_url,
        provider.protocol,
    );
    (label, detail)
}

/// 【首次配置】【自定义卡片】构造自定义兼容供应商的列表项与详情卡。
///
/// 返回:
/// - `(左侧显示名称, 右侧详情文本)`
pub(super) fn custom_provider_entry() -> (String, String) {
    let label = format!("+ {}", t("Custom provider", "自定义供应商"));
    let detail = format!(
        "{}\n\n{}: openai-chat / openai-responses / anthropic\n\n{}",
        t(
            "Connect any OpenAI-compatible, Anthropic-compatible, or self-hosted gateway.",
            "接入任意 OpenAI / Anthropic 兼容接口或自建模型网关。",
        ),
        t("Protocols", "支持协议"),
        t(
            "Enter / Double-click: Fill URL, key and model",
            "Enter / 双击: 填写地址、密钥和模型",
        ),
    );
    (label, detail)
}

/// 【首次配置】【底栏操作】根据当前高亮项生成可点击的快捷键提示栏。
///
/// 参数:
/// - `kind`: 当前高亮预设的就绪分类；自定义项传入 None
///
/// 返回:
/// - 带颜色分段的底栏文案
pub(super) fn provider_footer(kind: Option<&QuickStartKind>) -> String {
    match kind {
        Some(QuickStartKind::BuiltInFree) => help_line(&[
            ("↑↓", t("move", "移动")),
            ("Enter", t("start now", "立即开始")),
            ("m", t("models", "选模型")),
            ("e", t("edit", "编辑")),
            ("Esc", t("exit", "退出")),
        ]),
        Some(QuickStartKind::EnvReady) => help_line(&[
            ("↑↓", t("move", "移动")),
            ("Space", t("quick start", "一键启动")),
            ("Enter", t("configure", "配置")),
            ("Esc", t("exit", "退出")),
        ]),
        _ => help_line(&[
            ("↑↓", t("move", "移动")),
            ("Enter", t("configure", "配置")),
            ("Esc", t("exit", "退出")),
        ]),
    }
}

/// 【首次配置】【表单字段】按供应商类型排序字段，使预设模板打开后直接聚焦密钥输入框。
///
/// 参数:
/// - `provider`: 模板或自定义基础配置
/// - `existing`: 是否为内置模板
///
/// 返回:
/// - 带分组标题的表单字段集合
pub(super) fn provider_form_fields(provider: &ProviderConfig, existing: bool) -> Vec<Field> {
    let key_first = existing && !provider.is_opencode_zen();
    let name_field = Field::new(
        t("Provider name", "供应商名称"),
        provider.display_name.clone(),
    );
    let url_field = Field::new(t("API address", "API 地址"), provider.base_url.clone());
    let protocol_field = Field::new(t("Protocol", "协议"), provider.protocol.clone()).choices(&[
        "auto",
        "openai-chat",
        "openai-responses",
        "anthropic",
    ]);
    let key_field = Field::new(
        t("API key or $env:NAME", "密钥或 $env:NAME"),
        provider.api_key.clone().unwrap_or_default(),
    )
    .secret();

    if key_first {
        // 1. 预设模板已填好名称与地址，把凭据放在首屏焦点减少无意义翻行
        vec![
            Field::section(t("Credentials", "接入凭据")),
            key_field,
            Field::section(t("Endpoint", "连接参数")),
            name_field,
            url_field,
            protocol_field,
        ]
    } else {
        vec![
            Field::section(t("Endpoint", "连接参数")),
            name_field,
            url_field,
            protocol_field,
            Field::section(t("Credentials", "接入凭据")),
            key_field,
        ]
    }
}

/// 【首次配置】【字段解析】从分组表单提取供应商名称、地址、协议与密钥。
///
/// 参数:
/// - `fields`: 表单提交后的字段切片
/// - `existing`: 是否使用预设键优先布局
///
/// 返回:
/// - `(name, normalized_base_url, protocol, raw_api_key)`
pub(super) fn extract_provider_form(
    fields: &[Field],
    existing: bool,
    is_opencode_zen: bool,
) -> (String, String, String, String) {
    let key_first = existing && !is_opencode_zen;
    let (name_raw, url_raw, protocol_raw, key_raw) = if key_first {
        (
            fields[3].value.trim(),
            fields[4].value.trim(),
            fields[5].value.clone(),
            fields[1].value.clone(),
        )
    } else {
        (
            fields[1].value.trim(),
            fields[2].value.trim(),
            fields[3].value.clone(),
            fields[5].value.clone(),
        )
    };
    let mut base_url = url_raw.trim_end_matches('/').to_string();
    if let Some(base) = base_url
        .strip_suffix("/chat/completions")
        .or_else(|| base_url.strip_suffix("/responses"))
        .or_else(|| base_url.strip_suffix("/messages"))
    {
        base_url = base.into();
    }
    (name_raw.to_string(), base_url, protocol_raw, key_raw)
}

/// 【首次配置】【模型卡片】生成模型列表行标题与右侧元数据详情卡片。
///
/// 参数:
/// - `model`: 模型标识
/// - `provider`: 供应商配置
/// - `status`: 目录拉取状态摘要
///
/// 返回:
/// - `(列表显示标签, 右侧详情文本)`
pub(super) fn model_menu_entry(
    model: &str,
    provider: &ProviderConfig,
    status: &str,
) -> (String, String) {
    let is_default = !provider.default_model.trim().is_empty() && model == provider.default_model;
    let label = if is_default {
        format!("◆ {model}  [{}]", t("recommended", "推荐"))
    } else {
        format!("  {model}")
    };
    let mut detail_lines = vec![status.to_string(), String::new()];
    detail_lines.push(format!("{}: {model}", t("Model", "模型标识")));
    detail_lines.push(format!(
        "{}: {} ({})",
        t("Provider", "供应商"),
        provider.display_name,
        provider.protocol
    ));
    if let Some(meta) = provider.model_metadata.get(model) {
        append_model_metadata(&mut detail_lines, meta);
    }
    detail_lines.push(String::new());
    detail_lines.push(
        t(
            "Enter / Double-click: Confirm & start chatting\nm: Custom ID   f: Refresh catalog",
            "Enter / 双击: 确认并开始对话\nm: 手动填写模型   f: 刷新目录",
        )
        .to_string(),
    );
    (label, detail_lines.join("\n"))
}

/// 【首次配置】【元数据摘要】向详情卡片追加上下文、输出上限和推理标签。
fn append_model_metadata(lines: &mut Vec<String>, meta: &ModelMetadata) {
    let mut specs = Vec::new();
    if let Some(ctx) = meta.context_chars {
        specs.push(format!("{} ctx", format_compact_count(ctx)));
    }
    if let Some(max_out) = meta.max_output_tokens {
        specs.push(format!("{} out", format_compact_count(max_out as usize)));
    }
    if !specs.is_empty() {
        lines.push(format!("{}: {}", t("Limits", "容量"), specs.join(" · ")));
    }
    if !meta.tags.is_empty() {
        lines.push(format!("{}: {}", t("Tags", "标签"), meta.tags.join(", ")));
    }
    if !meta.thinking_levels.is_empty() {
        lines.push(format!(
            "{}: {}",
            t("Thinking", "思考"),
            meta.thinking_levels.join("/")
        ));
    }
}

fn format_compact_count(value: usize) -> String {
    if value >= 1_000_000 {
        format!("{}M", value / 1_000_000)
    } else if value >= 1_000 {
        format!("{}k", value / 1_000)
    } else {
        value.to_string()
    }
}

fn is_local_endpoint(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.contains("localhost") || lower.contains("127.0.0.1")
}

/// 【首次配置】【模型引导】生成模型步底栏与状态摘要。
pub(super) fn model_footer(can_confirm: bool) -> String {
    if can_confirm {
        help_line(&[
            ("↑↓", t("move", "移动")),
            ("Enter", t("confirm", "确认")),
            ("m", t("custom", "自定义")),
            ("f", t("refresh", "刷新")),
            ("Esc", t("back", "返回")),
        ])
    } else {
        help_line(&[
            ("Enter", t("custom", "自定义")),
            ("f", t("refresh", "刷新")),
            ("Esc", t("back", "返回")),
        ])
    }
}

/// 【首次配置】【调试文本】供测试断言纯文本标题与状态胶囊结构。
#[cfg(test)]
pub(super) fn plain_summary_contains_step(text: &str) -> String {
    super::ui::strip_ansi_plain(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;

    fn test_paths(dir: &std::path::Path) -> SaiPaths {
        SaiPaths {
            config_dir: dir.join("config"),
            config_file: dir.join("config/config.jsonc"),
            secrets_file: dir.join("config/secrets.jsonc"),
            skills_dir: dir.join("config/skills"),
            data_dir: dir.join("data"),
            cache_dir: dir.join("cache"),
            state_dir: dir.join("state"),
            pictures_dir: dir.join("pictures"),
            fish_hook_file: dir.join("fish/sai.fish"),
            bash_hook_file: dir.join("shell/bash-hook.sh"),
            zsh_hook_file: dir.join("shell/zsh-hook.zsh"),
            powershell_hook_file: dir.join("shell/sai.ps1"),
        }
    }

    /// 【首次配置测试】【快速分类】内置免费供应商归为免密即用，缺失环境变量则提示录入。
    #[test]
    fn quick_start_classifies_free_env_and_manual_providers() {
        let temp = tempfile::tempdir().unwrap();
        let paths = test_paths(temp.path());
        AppConfig::init_files(&paths).unwrap();
        let config = AppConfig::load(&paths).unwrap();

        let zen = config.provider(None).unwrap();
        assert_eq!(quick_start_kind(zen, &paths), QuickStartKind::BuiltInFree);

        let mut env_missing = ProviderConfig::default_openai();
        env_missing.api_key = Some(format!(
            "$env:SAI_TEST_MISSING_{}",
            uuid::Uuid::new_v4().simple()
        ));
        assert_eq!(
            quick_start_kind(&env_missing, &paths),
            QuickStartKind::NeedsInput
        );

        let mut with_key = ProviderConfig::default_openai();
        with_key.api_key = Some("sk-fixture-123".into());
        assert_eq!(
            quick_start_kind(&with_key, &paths),
            QuickStartKind::EnvReady
        );
    }

    /// 【首次配置测试】【字段排序】预设模板首屏聚焦凭据输入框，提取结果与端点信息一致。
    #[test]
    fn preset_form_puts_credentials_first_and_normalizes_endpoint() {
        let mut preset = ProviderConfig::default_openai();
        preset.base_url = "https://example.com/v1/chat/completions".into();
        preset.api_key = Some("sk-preset".into());

        let fields = provider_form_fields(&preset, true);
        assert!(fields[0].section);
        assert!(fields[1].secret);
        let (name, base_url, protocol, api_key) = extract_provider_form(&fields, true, false);
        assert_eq!(name, preset.display_name);
        assert_eq!(base_url, "https://example.com/v1");
        assert_eq!(protocol, preset.protocol);
        assert_eq!(api_key, "sk-preset");

        let custom = ProviderConfig::new_openai_compatible();
        let custom_fields = provider_form_fields(&custom, false);
        assert!(custom_fields[0].section);
        assert!(!custom_fields[1].secret);
        assert!(custom_fields[5].secret);
    }

    /// 【首次配置测试】【卡片格式】验证步骤胶囊与模型详情卡包含容量与快捷键提示。
    #[test]
    fn step_header_and_model_card_format_cleanly() {
        let subtitle = step_subtitle(1, 2, "Choose provider", Some("1 ready"), true);
        let plain = plain_summary_contains_step(&subtitle);
        assert!(plain.contains("1/2"));
        assert!(plain.contains("Choose provider"));
        assert!(plain.contains("1 ready"));

        let mut provider = ProviderConfig::default_openai();
        provider.model_metadata.insert(
            "gpt-4o-mini".into(),
            ModelMetadata {
                context_chars: Some(128_000),
                max_output_tokens: Some(16_000),
                tags: vec!["fast".into()],
                thinking_levels: vec!["low".into(), "high".into()],
                ..Default::default()
            },
        );
        let (label, detail) = model_menu_entry("gpt-4o-mini", &provider, "Catalog ready");
        assert!(label.contains("gpt-4o-mini"));
        assert!(detail.contains("128k ctx"));
        assert!(detail.contains("16k out"));
        assert!(detail.contains("low/high"));
    }
}
