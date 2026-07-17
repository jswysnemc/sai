use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::Value;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// 权限审计判定结果。
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AuditDecision {
    Requested,
    Approved,
    Allowed,
    Denied,
    Completed,
    Failed,
}

#[derive(Debug, Serialize)]
struct PermissionAuditEvent<'event> {
    timestamp_ms: u128,
    session_id: &'event str,
    mode: &'event str,
    tool: &'event str,
    decision: AuditDecision,
    arguments: &'event Value,
    detail: Option<&'event str>,
}

/// 会话级权限审计日志。
#[derive(Debug, Clone)]
pub(crate) struct PermissionAuditLog {
    path: PathBuf,
    session_id: String,
}

impl PermissionAuditLog {
    /// 创建会话级权限审计日志。
    ///
    /// 参数:
    /// - `path`: JSONL 审计日志文件
    /// - `session_id`: 会话标识
    ///
    /// 返回:
    /// - 审计日志写入器
    pub(crate) fn new(path: PathBuf, session_id: impl Into<String>) -> Self {
        Self {
            path,
            session_id: session_id.into(),
        }
    }

    /// 追加权限审计事件。
    ///
    /// 参数:
    /// - `mode`: 权限模式
    /// - `tool`: 工具名称
    /// - `decision`: 判定结果
    /// - `arguments`: 工具参数
    /// - `detail`: 可选结果或错误摘要
    ///
    /// 返回:
    /// - 写入结果
    pub(crate) fn append(
        &self,
        mode: &str,
        tool: &str,
        decision: AuditDecision,
        arguments: &Value,
        detail: Option<&str>,
    ) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let event = PermissionAuditEvent {
            timestamp_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            session_id: &self.session_id,
            mode,
            tool,
            decision,
            arguments,
            detail,
        };
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .with_context(|| {
                format!(
                    "failed to open permission audit log {}",
                    self.path.display()
                )
            })?;
        serde_json::to_writer(&mut file, &event)?;
        file.write_all(b"\n")?;
        Ok(())
    }
}
