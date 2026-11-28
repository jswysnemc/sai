//! 群组工作台运行期模型快照：统一读取 Leader 与 Worker 生效模型、思考等级、上下文窗口与实时占用。

use crate::cli::repl_chrome::{compress_home_prefix, format_token_k, short_model_name};
use crate::config::{AppConfig, InputStyle};
use crate::paths::SaiPaths;
use crate::state::StateStore;
use crate::super_agent::tools::role_runtime_config;
use std::path::Path;

/// 单个 Super-Agent 角色（Leader 或 Worker）的模型基础信息。
#[derive(Debug, Clone)]
pub(super) struct RoleModelInfo {
    /// 供应商标识
    pub(super) provider_id: String,
    /// 供应商展示名称
    pub(super) provider_name: String,
    /// 完整模型标识
    pub(super) model_id: String,
    /// 底栏与侧栏使用的末段短模型名
    pub(super) short_model: String,
    /// 思考等级（空值归一化为 auto）
    pub(super) thinking: String,
    /// 上下文窗口上限（tokens）
    pub(super) context_window_tokens: usize,
    /// 当前会话已用上下文估算（tokens）
    pub(super) context_used_tokens: usize,
    /// 上下文占用比例 `[0.0, ...]`
    pub(super) context_ratio: f32,
    /// 最近一轮缓存命中率
    pub(super) cache_hit_ratio: Option<f32>,
    /// 自动压缩阈值估算（tokens）
    pub(super) compaction_trigger_tokens: usize,
    /// 是否显式配置了独立角色模型覆盖
    pub(super) is_override: bool,
}

impl Default for RoleModelInfo {
    /// 构造默认占位模型信息。
    fn default() -> Self {
        Self {
            provider_id: String::new(),
            provider_name: String::new(),
            model_id: "-".to_string(),
            short_model: "-".to_string(),
            thinking: "auto".to_string(),
            context_window_tokens: 128_000,
            context_used_tokens: 0,
            context_ratio: 0.0,
            cache_hit_ratio: None,
            compaction_trigger_tokens: 102_400,
            is_override: false,
        }
    }
}

impl RoleModelInfo {
    /// 返回与主 REPL 底栏一致的上下文占用文案，如 `0.0%/200k` 或 `12.5%/200k cache 88%`。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 紧凑上下文占用字符串
    pub(super) fn context_status(&self) -> String {
        let pct = (self.context_ratio * 100.0).clamp(0.0, 999.9);
        let mut status = format!("{pct:.1}%/{}", format_token_k(self.context_window_tokens));
        if let Some(ratio) = self.cache_hit_ratio {
            status.push_str(&format!(" cache {:.0}%", (ratio * 100.0).clamp(0.0, 100.0)));
        }
        status
    }

    /// 返回 `provider/model` 完整标签，便于详情与提示展示。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 完整供应商与模型组合文本
    pub(super) fn full_label(&self) -> String {
        if self.provider_id.is_empty() {
            self.model_id.clone()
        } else {
            format!("{}/{}", self.provider_id, self.model_id)
        }
    }
}

/// 【群组终端】【模型快照】汇总当前群组 Leader / Worker 模型、上下文读数、输入样式与工作区信息。
#[derive(Debug, Clone)]
pub(super) struct GroupModelSnapshot {
    pub(super) leader: RoleModelInfo,
    pub(super) worker: RoleModelInfo,
    /// Worker 是否与 Leader 使用完全相同的供应商、模型与思考等级
    pub(super) worker_shares_leader: bool,
    pub(super) max_concurrent_workers: usize,
    pub(super) worker_use_worktree: bool,
    pub(super) input_style: InputStyle,
    pub(super) directory: String,
    pub(super) branch: Option<String>,
}

impl Default for GroupModelSnapshot {
    /// 构造默认模型快照。
    fn default() -> Self {
        Self {
            leader: RoleModelInfo::default(),
            worker: RoleModelInfo::default(),
            worker_shares_leader: true,
            max_concurrent_workers: 4,
            worker_use_worktree: true,
            input_style: InputStyle::Panel,
            directory: "?".to_string(),
            branch: None,
        }
    }
}

impl GroupModelSnapshot {
    /// 【群组终端】【模型探测】从本地配置与 Leader 会话存储读取最新模型及上下文快照。
    ///
    /// 参数:
    /// - `paths`: Sai 路径集合
    /// - `group_id`: 当前选中群组 ID（存在时用于读取 Leader 真实上下文占用）
    /// - `workspace_path`: 当前群组工作区路径（为空时回退当前工作目录）
    ///
    /// 返回:
    /// - 最新模型与工作区快照
    pub(super) fn probe(
        paths: &SaiPaths,
        group_id: Option<&str>,
        workspace_path: Option<&str>,
    ) -> Self {
        // 1. 加载全局配置并分别派生 Leader 与 Worker 的角色运行配置
        let config = AppConfig::load_or_default(paths).unwrap_or_default();
        let leader_cfg = role_runtime_config(&config, true);
        let worker_cfg = role_runtime_config(&config, false);
        let leader_override = !config.super_agent.leader_provider_id.trim().is_empty()
            && !config.super_agent.leader_model.trim().is_empty();
        let worker_override = !config.super_agent.worker_provider_id.trim().is_empty()
            && !config.super_agent.worker_model.trim().is_empty();

        let mut leader = extract_role_model(&leader_cfg, leader_override);
        let worker = extract_role_model(&worker_cfg, worker_override);

        // 2. 若当前群组的 Leader 会话已存在，补充实时上下文占用与缓存命中率
        if let Some(gid) = group_id {
            let session_id = format!("super-leader-{gid}");
            if let Ok(store) = StateStore::for_session(paths, &session_id) {
                if let Ok(snapshot) = store.session_snapshot(leader.context_window_tokens) {
                    leader.context_window_tokens = snapshot.context_window_tokens.max(1);
                    leader.context_ratio = snapshot.context_token_ratio;
                    leader.context_used_tokens =
                        (leader.context_ratio * leader.context_window_tokens as f32).round()
                            as usize;
                    leader.cache_hit_ratio = snapshot
                        .usage
                        .last_conversation_usage
                        .as_ref()
                        .filter(|usage| usage.prompt_tokens > 0)
                        .map(|usage| {
                            usage.cache_read_tokens.min(usage.prompt_tokens) as f32
                                / usage.prompt_tokens as f32
                        });
                    if let Ok(resolved) = store.resolve_compaction_policy(&leader_cfg.context) {
                        leader.compaction_trigger_tokens =
                            resolved.policy.trigger_chars(leader.context_window_tokens);
                    }
                }
            }
        }

        // 3. 解析当前群组工作区路径与 Git 分支，保持与主 REPL 底栏及侧栏一致
        let cwd_fallback = crate::runtime_cwd::current_dir().ok();
        let effective_path = workspace_path
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(Path::new)
            .filter(|path| path.is_dir())
            .map(Path::to_path_buf)
            .or(cwd_fallback);
        let directory = effective_path
            .as_ref()
            .map(|path| compress_home_prefix(&path.display().to_string()))
            .unwrap_or_else(|| "?".to_string());
        let branch = effective_path
            .as_ref()
            .and_then(|path| crate::agent::git_branch(path));

        let worker_shares_leader = leader.provider_id == worker.provider_id
            && leader.model_id == worker.model_id
            && leader.thinking == worker.thinking;

        Self {
            leader,
            worker,
            worker_shares_leader,
            max_concurrent_workers: config.super_agent.max_concurrent_workers,
            worker_use_worktree: config.super_agent.worker_use_worktree,
            input_style: config.display.input_style,
            directory,
            branch,
        }
    }

    /// 返回底栏右侧展示的目录与分支纯文本（形如 `~/workspace/sai  main`）。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 目录或 `目录  分支`
    pub(super) fn right_workspace_plain(&self) -> String {
        match self.branch.as_deref().filter(|branch| !branch.is_empty()) {
            Some(branch) => format!("{}  {branch}", self.directory),
            None => self.directory.clone(),
        }
    }
}

/// 从角色运行期配置提取供应商、模型短名、思考等级与上下文窗口信息。
///
/// 参数:
/// - `config`: 已应用角色覆盖的配置副本
/// - `is_override`: 是否来自 `super_agent.*` 独立角色模型配置
///
/// 返回:
/// - 角色模型基础信息
fn extract_role_model(config: &AppConfig, is_override: bool) -> RoleModelInfo {
    let provider = config.provider(None).ok();
    let provider_id = provider
        .map(|item| item.id.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| config.active_provider.trim().to_string());
    let provider_name = provider
        .map(|item| item.display_name.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| provider_id.clone());
    let model_id = provider
        .map(|item| item.default_model.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "-".to_string());
    let thinking = provider
        .map(|item| item.thinking_level.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "auto".to_string());
    let context_window_tokens = config
        .active_context_window_tokens()
        .unwrap_or(128_000)
        .max(1);
    let policy = crate::state::CompactionBudgetPolicy::from_context(
        config.context.clamped_compaction_ratio(),
        config.context.compaction_reserve_tokens,
    )
    .with_mode(config.context.compaction_mode);
    RoleModelInfo {
        provider_id,
        provider_name,
        short_model: short_model_name(&model_id),
        model_id,
        thinking,
        context_window_tokens,
        context_used_tokens: 0,
        context_ratio: 0.0,
        cache_hit_ratio: None,
        compaction_trigger_tokens: policy.trigger_chars(context_window_tokens),
        is_override,
    }
}

/// 将成员记录中的 `provider/model` 标签提取为紧凑末段模型名。
///
/// 参数:
/// - `model_label`: 成员表记录的模型标签
///
/// 返回:
/// - 末段短模型名；为空时返回空字符串
pub(super) fn short_member_model(model_label: &str) -> String {
    let trimmed = model_label.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    short_model_name(trimmed)
}
