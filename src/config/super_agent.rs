use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

/// 【超级智能体】【配置模型】定义双层架构（Leader/Router 与 Worker/Runner）、多渠道接入与 Watchdog 参数。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuperAgentConfig {
    /// 是否启用 Super-Agent 群组与编排运行时
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// 是否默认将外部渠道（微信/飞书/QQ/OneBot）消息交由群组 Leader 编排
    #[serde(default = "default_true")]
    pub channel_leader_routing: bool,
    /// 默认绑定的群组标识；为空时自动使用或创建默认群组
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_group_id: Option<String>,
    /// Leader(Router) 专用供应商；留空时继承当前默认供应商
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub leader_provider_id: String,
    /// Leader(Router) 专用模型；留空时继承当前默认模型
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub leader_model: String,
    /// Worker(Runner) 专用供应商；留空时继承当前默认供应商
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub worker_provider_id: String,
    /// Worker(Runner) 专用模型；留空时继承当前默认模型
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub worker_model: String,
    /// 同时运行的 Worker(Runner) 最大并发数
    #[serde(default = "default_max_concurrent_workers")]
    pub max_concurrent_workers: usize,
    /// Leader 单次批处理最多合并的 Trigger 数量（对应论文 2.1 节批处理上限）
    #[serde(default = "default_leader_batch_limit")]
    pub leader_batch_limit: usize,
    /// IM 渠道回复建议字数上限（对应论文 4.2 节低打扰规范）
    #[serde(default = "default_im_reply_max_words")]
    pub im_reply_max_words: usize,
    /// 是否启用后台 Watchdog 巡检回路（对应论文第 6 节）
    #[serde(default = "default_true")]
    pub watchdog_enabled: bool,
    /// Watchdog 巡检间隔（秒）
    #[serde(default = "default_watchdog_interval_seconds")]
    pub watchdog_interval_seconds: u64,
    /// Worker 无进展判定超时阈值（秒）
    #[serde(default = "default_worker_stall_timeout_seconds")]
    pub worker_stall_timeout_seconds: u64,
    /// Worker 是否默认尝试创建独立 Git Worktree 隔离工作目录
    #[serde(default = "default_true")]
    pub worker_use_worktree: bool,
}

impl Default for SuperAgentConfig {
    /// 构造默认 Super-Agent 配置。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 具备合理并发、批处理与监控默认值的配置
    fn default() -> Self {
        Self {
            enabled: true,
            channel_leader_routing: true,
            default_group_id: None,
            leader_provider_id: String::new(),
            leader_model: String::new(),
            worker_provider_id: String::new(),
            worker_model: String::new(),
            max_concurrent_workers: default_max_concurrent_workers(),
            leader_batch_limit: default_leader_batch_limit(),
            im_reply_max_words: default_im_reply_max_words(),
            watchdog_enabled: true,
            watchdog_interval_seconds: default_watchdog_interval_seconds(),
            worker_stall_timeout_seconds: default_worker_stall_timeout_seconds(),
            worker_use_worktree: true,
        }
    }
}

impl SuperAgentConfig {
    /// 判断是否为默认配置，供主配置序列化省略使用。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 与默认值一致时返回 true
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// 校验 Super-Agent 运行参数边界与模型对配。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 合法时返回 Ok
    pub fn validate(&self) -> Result<()> {
        if self.max_concurrent_workers == 0 || self.max_concurrent_workers > 32 {
            bail!("super_agent.max_concurrent_workers must be between 1 and 32");
        }
        if self.leader_batch_limit == 0 || self.leader_batch_limit > 64 {
            bail!("super_agent.leader_batch_limit must be between 1 and 64");
        }
        if self.im_reply_max_words < 20 || self.im_reply_max_words > 2000 {
            bail!("super_agent.im_reply_max_words must be between 20 and 2000");
        }
        if self.watchdog_interval_seconds < 5 || self.watchdog_interval_seconds > 3600 {
            bail!("super_agent.watchdog_interval_seconds must be between 5 and 3600");
        }
        if self.worker_stall_timeout_seconds < 30 || self.worker_stall_timeout_seconds > 86_400 {
            bail!("super_agent.worker_stall_timeout_seconds must be between 30 and 86400");
        }
        validate_model_pair(
            "super_agent.leader",
            &self.leader_provider_id,
            &self.leader_model,
        )?;
        validate_model_pair(
            "super_agent.worker",
            &self.worker_provider_id,
            &self.worker_model,
        )?;
        Ok(())
    }
}

/// 校验供应商与模型标识必须成对提供或同时留空。
///
/// 参数:
/// - `prefix`: 配置键前缀
/// - `provider_id`: 供应商标识
/// - `model`: 模型名称
///
/// 返回:
/// - 成对一致时返回 Ok
fn validate_model_pair(prefix: &str, provider_id: &str, model: &str) -> Result<()> {
    let has_provider = !provider_id.trim().is_empty();
    let has_model = !model.trim().is_empty();
    if has_provider != has_model {
        bail!("{prefix}_provider_id and {prefix}_model must be provided together");
    }
    Ok(())
}

/// 返回布尔默认值 true。
///
/// 参数:
/// - 无
///
/// 返回:
/// - true
fn default_true() -> bool {
    true
}

/// 返回默认最大 Worker 并发数。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 4
fn default_max_concurrent_workers() -> usize {
    4
}

/// 返回 Leader 单轮批处理默认最大条数。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 16
fn default_leader_batch_limit() -> usize {
    16
}

/// 返回 IM 渠道默认回复字数上限。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 120
fn default_im_reply_max_words() -> usize {
    120
}

/// 返回 Watchdog 默认巡检周期（秒）。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 30
fn default_watchdog_interval_seconds() -> u64 {
    30
}

/// 返回 Worker 无进展默认超时阈值（秒）。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 300
fn default_worker_stall_timeout_seconds() -> u64 {
    300
}
