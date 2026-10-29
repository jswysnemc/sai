use super::budget::{clamp_compaction_ratio, CompactionBudgetPolicy};
use crate::config::ContextConfig;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 会话状态目录里的压缩策略文件名。
const POLICY_FILE: &str = "compaction-policy.json";

/// 落在会话目录上的自动压缩策略。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionCompactionPolicy {
    #[serde(
        default,
        skip_serializing_if = "crate::config::CompactionMode::is_legacy"
    )]
    pub compaction_mode: crate::config::CompactionMode,
    pub compaction_ratio: f32,
    pub compaction_reserve_tokens: usize,
}

/// 解析后的策略，并标明是否为本会话覆盖。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResolvedCompactionPolicy {
    pub policy: CompactionBudgetPolicy,
    pub session_override: bool,
}

/// 会话压缩策略文件路径。
///
/// 参数:
/// - `state_dir`: 会话状态目录
///
/// 返回:
/// - 策略文件路径
pub fn policy_path(state_dir: &Path) -> PathBuf {
    state_dir.join(POLICY_FILE)
}

/// 读取会话覆盖；文件不存在时返回空。
///
/// 参数:
/// - `state_dir`: 会话状态目录
///
/// 返回:
/// - 已保存的策略
pub fn load_session_policy(state_dir: &Path) -> Result<Option<SessionCompactionPolicy>> {
    let path = policy_path(state_dir);
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let saved = serde_json::from_str::<SessionCompactionPolicy>(&raw)
        .with_context(|| format!("invalid JSON in {}", path.display()))?;
    Ok(Some(SessionCompactionPolicy {
        compaction_mode: saved.compaction_mode,
        compaction_ratio: clamp_compaction_ratio(saved.compaction_ratio),
        compaction_reserve_tokens: saved.compaction_reserve_tokens,
    }))
}

/// 写入本会话压缩策略。
///
/// 参数:
/// - `state_dir`: 会话状态目录
/// - `ratio`: 压缩比例
/// - `reserve_tokens`: 预留 token
///
/// 返回:
/// - 写入是否成功
pub fn save_session_policy(state_dir: &Path, ratio: f32, reserve_tokens: usize) -> Result<()> {
    save_budget_policy(
        state_dir,
        CompactionBudgetPolicy::from_context(ratio, reserve_tokens),
    )
}

/// 【上下文】【策略保存】原子保存模式和数值，旧调用仍通过双条件入口写入。
/// 参数：`state_dir` 为会话目录，`policy` 为已确认策略；返回：保存结果。
fn save_budget_policy(state_dir: &Path, policy: CompactionBudgetPolicy) -> Result<()> {
    let path = policy_path(state_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let value = SessionCompactionPolicy {
        compaction_mode: policy.mode,
        compaction_ratio: clamp_compaction_ratio(policy.ratio),
        compaction_reserve_tokens: policy.reserve_tokens,
    };
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    use std::io::Write;
    temporary.write_all(format!("{}\n", serde_json::to_string_pretty(&value)?).as_bytes())?;
    temporary.persist(&path)?;
    Ok(())
}

/// 删除本会话覆盖，回到全局默认。
///
/// 参数:
/// - `state_dir`: 会话状态目录
///
/// 返回:
/// - 删除是否成功
pub fn clear_session_policy(state_dir: &Path) -> Result<()> {
    let path = policy_path(state_dir);
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

/// 会话覆盖优先，否则用全局上下文配置。
///
/// 参数:
/// - `state_dir`: 会话状态目录
/// - `fallback`: 全局上下文配置
///
/// 返回:
/// - 解析后的策略
pub fn resolve_compaction_policy(
    state_dir: &Path,
    fallback: &ContextConfig,
) -> Result<ResolvedCompactionPolicy> {
    if let Some(saved) = load_session_policy(state_dir)? {
        return Ok(ResolvedCompactionPolicy {
            policy: CompactionBudgetPolicy::from_context(
                saved.compaction_ratio,
                saved.compaction_reserve_tokens,
            )
            .with_mode(saved.compaction_mode),
            session_override: true,
        });
    }
    Ok(ResolvedCompactionPolicy {
        policy: CompactionBudgetPolicy::from_context(
            fallback.clamped_compaction_ratio(),
            fallback.compaction_reserve_tokens,
        )
        .with_mode(fallback.compaction_mode),
        session_override: false,
    })
}

impl crate::state::StateStore {
    /// 【上下文】【策略保存】保存编辑器确认的单条件模式和参数。
    /// 参数：`policy` 为策略草稿；返回：持久化结果。
    pub fn save_compaction_budget_policy(&self, policy: CompactionBudgetPolicy) -> Result<()> {
        save_budget_policy(self.state_dir(), policy)
    }
    /// 读取当前会话生效的自动压缩策略。
    ///
    /// 参数:
    /// - `fallback`: 未覆盖时使用的全局配置
    ///
    /// 返回:
    /// - 夹紧后的比例与预留
    pub fn resolve_compaction_policy(
        &self,
        fallback: &ContextConfig,
    ) -> Result<ResolvedCompactionPolicy> {
        resolve_compaction_policy(self.state_dir(), fallback)
    }

    /// 写入本会话压缩策略。
    ///
    /// 参数:
    /// - `ratio`: 压缩比例
    /// - `reserve_tokens`: 预留 token
    ///
    /// 返回:
    /// - 写入是否成功
    pub fn save_compaction_policy(&self, ratio: f32, reserve_tokens: usize) -> Result<()> {
        save_session_policy(self.state_dir(), ratio, reserve_tokens)
    }

    /// 清除本会话覆盖。
    ///
    /// 返回:
    /// - 清除是否成功
    pub fn clear_compaction_policy(&self) -> Result<()> {
        clear_session_policy(self.state_dir())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ContextConfig;

    /// 【上下文】【单条件回归】保存重载、全局回退和旧文件分别保留明确模式与旧语义。
    /// 参数：无；返回：无。
    #[test]
    fn explicit_modes_round_trip_without_changing_legacy_policies() {
        use crate::config::CompactionMode;
        let temp = tempfile::tempdir().unwrap();
        let fallback = ContextConfig::default();
        let base = CompactionBudgetPolicy::from_context(0.9, 50_000);
        for (mode, trigger) in [
            (CompactionMode::Ratio, 72_000),
            (CompactionMode::Reserve, 30_000),
        ] {
            save_budget_policy(temp.path(), base.with_mode(mode)).unwrap();
            let resolved = resolve_compaction_policy(temp.path(), &fallback).unwrap();
            assert_eq!(resolved.policy.mode, mode);
            assert_eq!(resolved.policy.trigger_chars(80_000), trigger);
        }
        save_session_policy(temp.path(), 0.9, 50_000).unwrap();
        let resolved = resolve_compaction_policy(temp.path(), &fallback).unwrap();
        assert_eq!(resolved.policy.mode, CompactionMode::Legacy);
        assert_eq!(resolved.policy.trigger_chars(80_000), 72_000);
        clear_session_policy(temp.path()).unwrap();
        let fallback = ContextConfig {
            compaction_mode: CompactionMode::Reserve,
            ..fallback
        };
        assert_eq!(
            resolve_compaction_policy(temp.path(), &fallback)
                .unwrap()
                .policy
                .trigger_chars(80_000),
            30_000
        );
        for reserve in [0, 80_000, 100_000] {
            assert_eq!(
                CompactionBudgetPolicy::from_context(0.9, reserve)
                    .with_mode(CompactionMode::Reserve)
                    .trigger_chars(80_000),
                80_000
            );
        }
    }

    #[test]
    fn missing_file_falls_back_to_global() {
        let temp = tempfile::tempdir().unwrap();
        let fallback = ContextConfig {
            compaction_ratio: 0.85,
            compaction_reserve_tokens: 8_000,
            ..ContextConfig::default()
        };
        let resolved = resolve_compaction_policy(temp.path(), &fallback).unwrap();
        assert!(!resolved.session_override);
        assert!((resolved.policy.ratio - 0.85).abs() < f32::EPSILON);
        assert_eq!(resolved.policy.reserve_tokens, 8_000);
    }

    #[test]
    fn saved_policy_overrides_global() {
        let temp = tempfile::tempdir().unwrap();
        save_session_policy(temp.path(), 0.7, 2_000).unwrap();
        let fallback = ContextConfig {
            compaction_ratio: 0.9,
            compaction_reserve_tokens: 50_000,
            ..ContextConfig::default()
        };
        let resolved = resolve_compaction_policy(temp.path(), &fallback).unwrap();
        assert!(resolved.session_override);
        assert!((resolved.policy.ratio - 0.7).abs() < f32::EPSILON);
        assert_eq!(resolved.policy.reserve_tokens, 2_000);
    }
}
