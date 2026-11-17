//! Agent 档案右侧详情卡片摘要构造：基本信息、系统提示词、工具白名单与 Skills 暴露概览。

use super::is_builtin;
use crate::config::{AgentProfile, DEFERRED_ALL_NON_BASE};
use crate::config_tui::ui::truncate;
use crate::i18n::text as t;

/// 【Agent配置】【档案概览】组装 Agent 列表右侧展示的身份类型、工具与 Skills 摘要。
///
/// 参数:
/// - `profile`: Agent 档案
///
/// 返回:
/// - 多行概览文本
pub(super) fn profile_overview(profile: &AgentProfile) -> String {
    let kind = if is_builtin(&profile.id) {
        t("Built-in profile", "内置档案")
    } else {
        t("Custom profile — d to delete.", "自定义档案 — d 删除。")
    };
    format!(
        "{kind}\n\n{}\n{}",
        tools_summary(profile),
        skills_summary(profile)
    )
}

/// 【Agent配置】【基本信息摘要】返回基本信息分区的当前模型、思考等级与注册状态摘要。
pub(super) fn basics_summary(profile: &AgentProfile) -> String {
    let model = if profile.provider_id.is_empty() {
        t("inherit current model", "沿用当前模型").to_string()
    } else if profile.model.is_empty() {
        profile.provider_id.clone()
    } else {
        format!("{} / {}", profile.provider_id, profile.model)
    };
    let description = if profile.description.trim().is_empty() {
        t("(no description)", "（无描述）").to_string()
    } else {
        profile.description.clone()
    };
    format!(
        "{description}\n\n{}: {model}\n{}: {}\n{}: {} · AGENT.md: {}",
        t("Model", "模型"),
        t("Thinking", "思考等级"),
        profile.thinking_level,
        t("Register to main", "注册主 Agent"),
        bool_text(profile.register_to_main),
        bool_text(profile.load_instruction_files),
    )
}

/// 【Agent配置】【提示词摘要】返回系统提示词字符数及前段预览。
pub(super) fn prompt_summary(profile: &AgentProfile) -> String {
    let prompt = profile.system_prompt.trim();
    if prompt.is_empty() {
        t(
            "Empty — the agent uses the built-in prompt. Enter opens $EDITOR.",
            "为空 — 使用内置提示词。Enter 打开 $EDITOR 编辑。",
        )
        .to_string()
    } else {
        format!(
            "{} {} · {}\n\n{}",
            prompt.chars().count(),
            t("chars", "字符"),
            t("Enter opens $EDITOR", "Enter 打开 $EDITOR"),
            truncate(&prompt.replace('\n', " "), 160)
        )
    }
}

/// 【Agent配置】【工具能力摘要】区分全量继承、白名单数量与延迟加载工具统计。
pub(super) fn tools_summary(profile: &AgentProfile) -> String {
    let wildcard = profile
        .deferred_tools
        .iter()
        .any(|name| name == DEFERRED_ALL_NON_BASE);
    let deferred_count = profile
        .deferred_tools
        .iter()
        .filter(|name| name.as_str() != DEFERRED_ALL_NON_BASE)
        .count();
    let base = if profile.enabled_tools.is_empty() {
        t("Tools: inherit all", "工具：继承全量").to_string()
    } else {
        format!(
            "{}: {} {}",
            t("Tools", "工具"),
            profile.enabled_tools.len(),
            t("whitelisted", "项白名单")
        )
    };
    let deferred = if wildcard {
        t("all non-base tools deferred (*)", "非基础工具全部延迟（*）").to_string()
    } else if deferred_count > 0 {
        format!("{deferred_count} {}", t("deferred", "项延迟"))
    } else {
        t("none deferred", "无延迟").to_string()
    };
    format!("{base} · {deferred}")
}

/// 【Agent配置】【Skills摘要】返回 Skills 完整暴露与仅名称暴露数量。
pub(super) fn skills_summary(profile: &AgentProfile) -> String {
    if profile.skills_full.is_empty() && profile.skills_named.is_empty() {
        t(
            "Skills: not restricted (all visible when no capability override is set)",
            "Skills：未单独配置（无其他能力限制时全部可见）",
        )
        .to_string()
    } else {
        format!(
            "Skills: {} {} · {} {}",
            profile.skills_full.len(),
            t("full", "完整"),
            profile.skills_named.len(),
            t("name only", "仅名称")
        )
    }
}

fn bool_text(value: bool) -> &'static str {
    if value {
        t("yes", "是")
    } else {
        t("no", "否")
    }
}
