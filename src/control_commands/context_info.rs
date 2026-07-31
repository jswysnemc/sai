use crate::config::AppConfig;
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::Result;

/// 组装当前会话的上下文信息文本（供 /context 展示）。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - 多行上下文信息
pub fn context_info_text(paths: &SaiPaths) -> Result<String> {
    let config = AppConfig::load_or_default(paths)?;
    let state = crate::state::StateStore::new(paths)?;
    let session = crate::state::active_session(paths)?;
    let context_limit = config.active_context_window_tokens().unwrap_or(128_000);
    let snapshot = state.session_snapshot(context_limit)?;
    let provider = config.provider(None).ok();
    let model = provider
        .map(|item| item.default_model.trim().to_string())
        .filter(|model| !model.is_empty())
        .unwrap_or_else(|| "-".to_string());
    let thinking = provider
        .map(|item| item.thinking_level.trim().to_string())
        .filter(|level| !level.is_empty())
        .unwrap_or_else(|| "auto".to_string());
    let directory = crate::runtime_cwd::current_dir()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "?".to_string());

    let mut lines = vec![
        format!(
            "{}: {} ({})",
            t("session", "会话"),
            session.title,
            session.id
        ),
        format!(
            "{}: {} · {}: {}",
            t("turns", "轮次"),
            snapshot.turn_count,
            t("checkpoints", "检查点"),
            snapshot.checkpoint_count
        ),
        format!(
            "{}: {}",
            t("engine", "对话内核"),
            config.agent.engine.display_label()
        ),
        // 外部内核自带模型与上下文管理，sai 这边的数值不代表实际用量
        if config.agent.engine.is_external() {
            format!(
                "{}: {}",
                t("model", "模型"),
                t("managed by the external engine", "由外部内核自行管理")
            )
        } else {
            format!("{}: {model} ({thinking})", t("model", "模型"))
        },
        if config.agent.engine.is_external() {
            format!(
                "{}: {}",
                t("context window", "上下文窗口"),
                t("managed by the external engine", "由外部内核自行管理")
            )
        } else {
            format!(
                "{}: {} / {} tokens ({:.1}%)",
                t("context window", "上下文窗口"),
                snapshot.context_prompt_tokens,
                snapshot.context_window_tokens,
                (snapshot.context_token_ratio * 100.0).clamp(0.0, 999.9)
            )
        },
        format!(
            "{}: {} {} · {} prompt · {} completion",
            t("session usage", "会话用量"),
            snapshot.usage.requests,
            t("requests", "次请求"),
            snapshot.usage.prompt_tokens,
            snapshot.usage.completion_tokens
        ),
    ];
    // 1. 有压缩记录时补充压缩状态
    if let Some(compaction) = &snapshot.compaction {
        lines.push(format!(
            "{}: {} {}",
            t("compaction", "压缩"),
            compaction.compacted_turns,
            t("turns compacted", "轮已压缩")
        ));
    }
    // 2. 外部内核会让一批 sai 功能停用，在这里说清楚而不是让用户自己发现
    let unavailable = config.agent.engine.unavailable_features();
    if !unavailable.is_empty() {
        lines.push(format!(
            "{}: {}",
            t("disabled by engine", "内核导致停用"),
            unavailable.join("、")
        ));
    }
    lines.push(format!("{}: {directory}", t("directory", "工作目录")));
    Ok(lines.join("\n"))
}
