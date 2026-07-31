use super::command_policy::requires_sandbox_escape;
use super::path_policy::{
    contains_external_path, contains_sensitive_read_path, path_is_within_workspace,
    resolve_without_io,
};
use super::{AuditDecision, PermissionAuditLog};
use crate::tools::ToolPermission;
use anyhow::{bail, Result};
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

/// 工具权限策略模式。
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum PermissionProfileMode {
    Yolo,
    Audited,
    AutoAudit,
    Plan,
}

impl PermissionProfileMode {
    /// 返回用于协议和审计的稳定模式名称。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 小写模式名称
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Yolo => "yolo",
            Self::Audited => "audited",
            Self::AutoAudit => "auto_audit",
            Self::Plan => "plan",
        }
    }

    /// 编码为原子状态值。
    pub(crate) fn as_u8(self) -> u8 {
        match self {
            Self::Yolo => 0,
            Self::Audited => 1,
            Self::AutoAudit => 2,
            Self::Plan => 3,
        }
    }

    /// 从原子状态值解码。
    pub(crate) fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Audited,
            2 => Self::AutoAudit,
            3 => Self::Plan,
            _ => Self::Yolo,
        }
    }
}

/// 单个工具注册表绑定的权限配置。
#[derive(Debug, Clone)]
pub(crate) struct PermissionProfile {
    /// 可热更新的权限模式（Shift+Tab 立即生效）
    mode: Arc<AtomicU8>,
    workspace: PathBuf,
    audit: Option<PermissionAuditLog>,
    approved: Arc<Mutex<HashSet<String>>>,
}

impl PermissionProfile {
    /// 记录等待用户处理的权限请求。
    ///
    /// 参数:
    /// - `tool`: 工具名称
    /// - `arguments`: 工具参数
    ///
    /// 返回:
    /// - 无
    pub(crate) fn record_requested(&self, tool: &str, arguments: &Value) {
        self.record(tool, AuditDecision::Requested, arguments, None);
    }

    /// 记录批准的权限请求。
    ///
    /// 参数:
    /// - `tool`: 工具名称
    /// - `arguments`: 工具参数
    /// - `detail`: 批准说明；自动审核会给出放行理由，人工批准为 `None`
    ///
    /// 返回:
    /// - 无
    pub(crate) fn record_approved(&self, tool: &str, arguments: &Value, detail: Option<&str>) {
        self.approve_once(tool, arguments);
        self.record(tool, AuditDecision::Approved, arguments, detail);
    }

    /// 记录用户拒绝的权限请求。
    ///
    /// 参数:
    /// - `tool`: 工具名称
    /// - `arguments`: 工具参数
    /// - `reply`: 可选拒绝回复
    ///
    /// 返回:
    /// - 无
    pub(crate) fn record_denied(&self, tool: &str, arguments: &Value, reply: Option<&str>) {
        self.record(tool, AuditDecision::Denied, arguments, reply);
    }

    /// 创建工具权限配置。
    ///
    /// 参数:
    /// - `mode`: 权限策略模式
    /// - `workspace`: 允许写入的工作区根目录
    /// - `audit`: 非 YOLO 模式使用的审计日志
    ///
    /// 返回:
    /// - 权限配置
    pub(crate) fn new(
        mode: PermissionProfileMode,
        workspace: PathBuf,
        audit: Option<PermissionAuditLog>,
    ) -> Self {
        Self {
            mode: Arc::new(AtomicU8::new(mode.as_u8())),
            workspace: workspace.canonicalize().unwrap_or(workspace),
            audit,
            approved: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// 立即切换权限模式（无需重建工具注册表）。
    ///
    /// 参数:
    /// - `mode`: 新权限模式
    ///
    /// 返回:
    /// - 无
    pub(crate) fn set_mode(&self, mode: PermissionProfileMode) {
        self.mode.store(mode.as_u8(), Ordering::SeqCst);
    }

    /// 返回可与 Agent 共享的模式原子句柄。
    pub(crate) fn mode_handle(&self) -> Arc<AtomicU8> {
        self.mode.clone()
    }

    /// 判断当前是否为自动审核模式。
    ///
    /// 外部内核走的是另一条审核路径，需要据此决定是否并行发起 LLM 审核，
    /// 否则用户配了自动审核却只在自带内核下生效，行为不一致。
    ///
    /// 返回:
    /// - 处于 AutoAudit 模式时为 true
    pub(crate) fn is_auto_audit(&self) -> bool {
        self.current_mode() == PermissionProfileMode::AutoAudit
    }

    /// 读取当前权限模式。
    fn current_mode(&self) -> PermissionProfileMode {
        PermissionProfileMode::from_u8(self.mode.load(Ordering::SeqCst))
    }

    /// 标记指定工具调用已获得一次性用户批准。
    ///
    /// 参数:
    /// - `tool`: 工具名称
    /// - `arguments`: 工具参数
    ///
    /// 返回:
    /// - 无
    fn approve_once(&self, tool: &str, arguments: &Value) {
        if let Ok(mut approved) = self.approved.lock() {
            approved.insert(approval_key(tool, arguments));
        }
    }

    /// 消费指定工具调用的一次性批准状态。
    ///
    /// 参数:
    /// - `tool`: 工具名称
    /// - `arguments`: 工具参数
    ///
    /// 返回:
    /// - 存在匹配批准状态时返回 `true`
    fn consume_approval(&self, tool: &str, arguments: &Value) -> bool {
        self.approved
            .lock()
            .map(|mut approved| approved.remove(&approval_key(tool, arguments)))
            .unwrap_or(false)
    }

    /// 在工具执行前完成权限判定并写入审计日志。
    ///
    /// 参数:
    /// - `tool`: 工具名称
    /// - `permission`: 工具声明的权限等级
    /// - `arguments`: 工具参数
    ///
    /// 返回:
    /// - 是否需要启用 Shell 沙盒
    pub(crate) fn authorize(
        &self,
        tool: &str,
        permission: ToolPermission,
        arguments: &Value,
    ) -> Result<bool> {
        // 1. YOLO 模式不增加权限检查和沙盒
        if self.current_mode() == PermissionProfileMode::Yolo {
            return Ok(false);
        }
        let approved = self.consume_approval(tool, arguments);
        // 2. TODO 仅维护会话计划，不参与权限审计交互或日志记录
        if matches!(
            self.current_mode(),
            PermissionProfileMode::Audited | PermissionProfileMode::AutoAudit
        ) && tool == "todo"
        {
            return Ok(false);
        }
        // 3. 规划模式和审计模式先阻止不允许的工具类别
        if self.current_mode() == PermissionProfileMode::Plan
            && permission == ToolPermission::Writes
        {
            self.record(
                tool,
                AuditDecision::Denied,
                arguments,
                Some("plan mode blocks writes"),
            );
            bail!("Plan mode blocked non-read-only tool: {tool}")
        }
        if matches!(
            self.current_mode(),
            PermissionProfileMode::Audited | PermissionProfileMode::AutoAudit
        ) && tool == "background_command"
            && !approved
        {
            self.record(
                tool,
                AuditDecision::Denied,
                arguments,
                Some("background commands are unavailable in audited sandbox mode"),
            );
            bail!("permission audit blocked background command outside the foreground sandbox")
        }
        let external_path = contains_external_path(&self.workspace, arguments);
        let protected_read = contains_sensitive_read_path(&self.workspace, arguments);
        // 4. 外部读取和敏感读取必须经过交互批准；批准后允许本次调用继续
        if permission == ToolPermission::ReadOnly && (external_path || protected_read) && !approved
        {
            self.record(
                tool,
                AuditDecision::Denied,
                arguments,
                Some("interactive approval required for external or sensitive path access"),
            );
            bail!("permission audit requires interactive approval for path access")
        }
        // 5. 写入工具必须先通过工作区路径边界检查，批准的外部写入放行一次
        if permission == ToolPermission::Writes && !approved {
            self.ensure_workspace_paths(arguments)
                .inspect_err(|error| {
                    self.record(
                        tool,
                        AuditDecision::Denied,
                        arguments,
                        Some(&error.to_string()),
                    );
                })?;
        }
        // 6. 需要逃逸沙箱但未获批准时拒绝，避免沙箱内静默失败
        let escape_sandbox = tool == "run_command" && requires_sandbox_escape(arguments);
        if matches!(
            self.current_mode(),
            PermissionProfileMode::Audited | PermissionProfileMode::AutoAudit
        ) && escape_sandbox
            && !approved
        {
            self.record(
                tool,
                AuditDecision::Denied,
                arguments,
                Some("interactive approval required to leave workspace sandbox"),
            );
            bail!("permission audit requires interactive approval for elevated sandbox access")
        }
        // 7. 已批准的提升命令在沙箱外执行，其他前台命令保留工作区沙箱
        self.record(tool, AuditDecision::Allowed, arguments, None);
        Ok(matches!(
            self.current_mode(),
            PermissionProfileMode::Audited | PermissionProfileMode::AutoAudit
        ) && tool == "run_command"
            && cfg!(target_os = "linux")
            && !(approved && escape_sandbox))
    }

    /// 判断工具执行前是否需要等待用户完成交互式审计。
    ///
    /// 参数:
    /// - `tool`: 工具名称
    /// - `permission`: 工具声明的权限等级
    /// - `arguments`: 工具参数
    ///
    /// 返回:
    /// - 审计模式下写入工具或工作区外、敏感路径读取工具返回 `true`
    pub(crate) fn requires_interactive_audit(
        &self,
        tool: &str,
        permission: ToolPermission,
        arguments: &Value,
    ) -> bool {
        if !matches!(
            self.current_mode(),
            PermissionProfileMode::Audited | PermissionProfileMode::AutoAudit
        ) || tool == "todo"
        {
            return false;
        }
        if permission == ToolPermission::Writes {
            return true;
        }
        // run_command 需要逃逸沙箱（网络 / 包管理 / 家目录）时必须先交互批准
        if tool == "run_command" && requires_sandbox_escape(arguments) {
            return true;
        }
        permission == ToolPermission::ReadOnly
            && (contains_external_path(&self.workspace, arguments)
                || contains_sensitive_read_path(&self.workspace, arguments))
    }

    /// 记录工具最终执行结果。
    ///
    /// 参数:
    /// - `tool`: 工具名称
    /// - `arguments`: 工具参数
    /// - `result`: 工具执行结果
    ///
    /// 返回:
    /// - 无
    pub(crate) fn record_result(
        &self,
        tool: &str,
        arguments: &Value,
        result: std::result::Result<&str, &anyhow::Error>,
    ) {
        if self.current_mode() == PermissionProfileMode::Yolo
            || (matches!(
                self.current_mode(),
                PermissionProfileMode::Audited | PermissionProfileMode::AutoAudit
            ) && tool == "todo")
        {
            return;
        }
        match result {
            Ok(output) => self.record(
                tool,
                AuditDecision::Completed,
                arguments,
                Some(&truncate_detail(output)),
            ),
            Err(error) => self.record(
                tool,
                AuditDecision::Failed,
                arguments,
                Some(&truncate_detail(&error.to_string())),
            ),
        }
    }

    /// 校验显式路径参数没有逃逸工作区。
    ///
    /// 参数:
    /// - `arguments`: 工具参数
    ///
    /// 返回:
    /// - 所有路径均位于工作区时成功
    fn ensure_workspace_paths(&self, arguments: &Value) -> Result<()> {
        // 1. 校验常见独立路径字段
        for key in ["path", "file", "target", "destination", "cwd"] {
            let Some(value) = arguments.get(key).and_then(Value::as_str) else {
                continue;
            };
            let resolved = resolve_without_io(&self.workspace, Path::new(value));
            if !path_is_within_workspace(&self.workspace, &resolved) {
                bail!(
                    "permission audit blocked path outside workspace: {}",
                    resolved.display()
                )
            }
        }
        // 2. 校验 Patch 中的源路径、目标路径和移动目标
        if let Some(patch) = arguments.get("patch").and_then(Value::as_str) {
            for line in patch.lines() {
                if let Some(destination) = line.strip_prefix("*** Move to: ") {
                    let destination =
                        resolve_without_io(&self.workspace, Path::new(destination.trim()));
                    if !path_is_within_workspace(&self.workspace, &destination) {
                        bail!(
                            "permission audit blocked patch destination outside workspace: {}",
                            destination.display()
                        )
                    }
                    continue;
                }
                let Some((_, value)) = line.split_once(" File: ") else {
                    continue;
                };
                let path = value
                    .split_once(" -> ")
                    .map(|(source, _)| source)
                    .unwrap_or(value);
                let resolved = resolve_without_io(&self.workspace, Path::new(path.trim()));
                if !path_is_within_workspace(&self.workspace, &resolved) {
                    bail!(
                        "permission audit blocked patch path outside workspace: {}",
                        resolved.display()
                    )
                }
                if let Some((_, destination)) = value.split_once(" -> ") {
                    let destination =
                        resolve_without_io(&self.workspace, Path::new(destination.trim()));
                    if !path_is_within_workspace(&self.workspace, &destination) {
                        bail!(
                            "permission audit blocked patch destination outside workspace: {}",
                            destination.display()
                        )
                    }
                }
            }
        }
        Ok(())
    }

    /// 追加审计事件，审计写入失败不改变工具判定结果。
    ///
    /// 参数:
    /// - `tool`: 工具名称
    /// - `decision`: 审计决定
    /// - `arguments`: 工具参数
    /// - `detail`: 可选结果摘要
    ///
    /// 返回:
    /// - 无
    fn record(&self, tool: &str, decision: AuditDecision, arguments: &Value, detail: Option<&str>) {
        if let Some(audit) = &self.audit {
            let _ = audit.append(
                self.current_mode().as_str(),
                tool,
                decision,
                arguments,
                detail,
            );
        }
    }
}

/// 生成稳定的一次性批准键。
///
/// 参数:
/// - `tool`: 工具名称
/// - `arguments`: 工具参数
///
/// 返回:
/// - 由工具名称和规范化参数组成的键
fn approval_key(tool: &str, arguments: &Value) -> String {
    format!(
        "{tool}\n{}",
        serde_json::to_string(arguments).unwrap_or_default()
    )
}

/// 限制审计结果摘要长度。
///
/// 参数:
/// - `value`: 原始结果文本
///
/// 返回:
/// - 最多五百字符的摘要
fn truncate_detail(value: &str) -> String {
    value.chars().take(500).collect()
}

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
