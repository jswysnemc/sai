use crate::agent_engine::EventSender;
use crate::config::AppConfig;
use crate::permission::PermissionProfile;
use crate::tools::ToolPermission;
use anyhow::{Context, Result};
use serde_json::json;
use std::path::{Path, PathBuf};

/// 外部内核操作工作区时套用的治理规则。
///
/// 外部 agent 通过 ACP 把文件写入与命令执行交回给 sai，这里是它们落地前的关卡：
/// 复用与 sai 自带工具完全相同的权限判定、沙箱与输出压缩，
/// 换内核不等于绕过治理。
#[derive(Clone)]
pub(crate) struct AcpGovernance {
    workspace: PathBuf,
    profile: Option<PermissionProfile>,
    config: AppConfig,
    /// Sai 路径，用于解析当前可见的 Skills
    paths: Option<crate::paths::SaiPaths>,
    /// 会话标识；权限卡按此归属，缺了它 Web 端按会话查不到待审请求
    session_id: String,
    /// 自动审核运行时；仅在 auto_audit 模式且客户端可用时存在
    auto_audit: Option<super::audit::AutoAuditRuntime>,
    /// ACP 会话标识存储；无状态目录时为 None
    session_store: Option<super::session_store::AcpSessionStore>,
}

impl AcpGovernance {
    /// 创建治理句柄。
    ///
    /// 参数:
    /// - `workspace`: 工作区根目录
    /// - `profile`: 权限配置；YOLO 模式下没有
    /// - `config`: 应用配置，提供 shell 与输出过滤设置
    /// - `session_id`: 当前会话标识
    /// - `paths`: Sai 路径，用于构造自动审核客户端
    /// - `state_dir`: 会话状态目录，用于记住外部 agent 的会话标识
    ///
    /// 返回:
    /// - 治理句柄
    pub(crate) fn new(
        workspace: PathBuf,
        profile: Option<PermissionProfile>,
        config: AppConfig,
        session_id: String,
        paths: Option<&crate::paths::SaiPaths>,
        state_dir: Option<&std::path::Path>,
    ) -> Self {
        // 自动审核模式下预备审核客户端；构造失败时静默退回人工审核，
        // 与自带内核的处理一致，不因为审核模型不可用就阻断会话
        let auto_audit = match (&profile, paths) {
            (Some(profile), Some(paths)) if profile.is_auto_audit() => {
                crate::permission::resolve_auto_audit_client(&config, paths)
                    .ok()
                    .map(|client| super::audit::AutoAuditRuntime {
                        client,
                        context: crate::i18n::text(
                            "The operation comes from an external ACP agent running in this workspace.",
                            "该操作来自在本工作区运行的外部 ACP 内核。",
                        )
                        .to_string(),
                    })
            }
            _ => None,
        };
        Self {
            workspace,
            profile,
            config,
            paths: paths.cloned(),
            session_id,
            auto_audit,
            session_store: state_dir.map(super::session_store::AcpSessionStore::new),
        }
    }

    /// 返回 ACP 会话标识存储。
    ///
    /// 返回:
    /// - 存储句柄；无会话状态目录时为 None
    pub(crate) fn session_store(&self) -> Option<&super::session_store::AcpSessionStore> {
        self.session_store.as_ref()
    }

    /// 返回 ACP 会话上下文使用的应用配置。
    ///
    /// 返回:
    /// - 当前 Agent 已应用运行期覆盖后的配置
    pub(crate) fn config(&self) -> &AppConfig {
        &self.config
    }

    /// 返回用于发现 Skills 的 Sai 路径。
    ///
    /// 返回:
    /// - 正常会话中的路径；独立测试治理句柄为 None
    pub(crate) fn paths(&self) -> Option<&crate::paths::SaiPaths> {
        self.paths.as_ref()
    }

    /// 返回 Sai 会话标识。
    ///
    /// 返回:
    /// - 权限与结构化提问使用的宿主会话标识
    pub(crate) fn session_id(&self) -> &str {
        &self.session_id
    }

    /// 校验一次文件写入是否允许。
    ///
    /// 走 `PermissionProfile::authorize`，因此工作区边界、符号链接逃逸、
    /// 敏感路径的判定与 sai 自带的 write_file 工具完全一致，审计日志也照常落盘。
    ///
    /// 参数:
    /// - `path`: 目标路径
    /// - `events`: 事件发送端，用于呈现权限卡
    ///
    /// 返回:
    /// - 允许时为 Ok；被拒或越界时返回错误
    pub(crate) async fn authorize_write(&self, path: &Path, events: &EventSender) -> Result<()> {
        let Some(profile) = &self.profile else {
            // 未绑定权限配置（YOLO）时不额外设限，与自带工具的行为一致
            return Ok(());
        };
        let arguments = json!({ "path": path.display().to_string() });
        // 1. 先走人工审核并写入批准记录，否则下一步的 authorize 找不到记录会直接拒绝
        super::audit::ensure_authorized(
            profile,
            &self.session_id,
            "write_file",
            &arguments,
            ToolPermission::Writes,
            events,
            self.auto_audit.as_ref(),
        )
        .await?;
        // 2. 再做静态边界校验：工作区外与符号链接逃逸即便获批也不放行
        profile
            .authorize("write_file", ToolPermission::Writes, &arguments)
            .with_context(|| format!("ACP agent write was blocked by sai: {}", path.display()))?;
        profile.record_result("write_file", &arguments, Ok(""));
        Ok(())
    }

    /// 校验一次文件读取是否允许。
    ///
    /// 工作区外路径和敏感路径沿用内置 read_file 的交互审核规则，
    /// 工作区内普通读取无需额外确认。
    ///
    /// 参数:
    /// - `path`: 目标路径
    /// - `events`: 事件发送端，用于呈现权限卡
    ///
    /// 返回:
    /// - 允许时为 Ok；被拒时返回错误
    pub(crate) async fn authorize_read(&self, path: &Path, events: &EventSender) -> Result<()> {
        let Some(profile) = &self.profile else {
            return Ok(());
        };
        let arguments = json!({ "path": path.display().to_string() });
        // 1. 仅在策略要求时发起交互，普通工作区读取直接继续
        if profile.requires_interactive_audit(
            "read_file",
            ToolPermission::ReadOnly,
            &arguments,
        ) {
            super::audit::ensure_authorized(
                profile,
                &self.session_id,
                "read_file",
                &arguments,
                ToolPermission::ReadOnly,
                events,
                self.auto_audit.as_ref(),
            )
            .await?;
        }
        // 2. 执行最终边界和敏感路径判定，并写入统一审计记录
        profile
            .authorize("read_file", ToolPermission::ReadOnly, &arguments)
            .with_context(|| format!("ACP agent read was blocked by sai: {}", path.display()))?;
        Ok(())
    }

    /// 判断命令是否需要在沙箱内执行。
    ///
    /// 参数:
    /// - `command`: 待执行命令
    /// - `cwd`: 命令工作目录
    /// - `events`: 事件发送端，用于呈现权限卡
    ///
    /// 返回:
    /// - 需要沙箱时为 true
    pub(crate) async fn authorize_command(
        &self,
        command: &str,
        cwd: &Path,
        events: &EventSender,
    ) -> Result<bool> {
        let Some(profile) = &self.profile else {
            return Ok(false);
        };
        let arguments = json!({
            "command": command,
            "cwd": cwd.display().to_string()
        });
        super::audit::ensure_authorized(
            profile,
            &self.session_id,
            "run_command",
            &arguments,
            ToolPermission::Writes,
            events,
            self.auto_audit.as_ref(),
        )
        .await?;
        let sandboxed = profile
            .authorize("run_command", ToolPermission::Writes, &arguments)
            .with_context(|| format!("ACP agent command was blocked by sai: {command}"))?;
        Ok(sandboxed)
    }

    /// 记录命令执行结果，保持审计日志完整。
    ///
    /// 参数:
    /// - `command`: 已执行的命令
    /// - `output`: 命令输出
    ///
    /// 返回:
    /// - 无
    pub(crate) fn record_command_result(&self, command: &str, output: &str) {
        let Some(profile) = &self.profile else {
            return;
        };
        profile.record_result("run_command", &json!({ "command": command }), Ok(output));
    }

    /// 按配置把命令改写为 rtk 代理形式。
    ///
    /// 外部内核的命令输出同样会进入上下文，不压缩的话长输出照样灌满窗口——
    /// 与自带 run_command 用同一套判定，避免两条路径的压缩行为不一致。
    ///
    /// 参数:
    /// - `command`: 原始命令
    ///
    /// 返回:
    /// - 需要改写时返回新命令，否则返回原命令
    pub(crate) fn apply_output_filter(&self, command: &str) -> String {
        crate::tools::command::rewrite_command(
            command,
            &self.config.tools.command_filter,
            &self.config.tools.command_filter_denylist,
        )
        .unwrap_or_else(|| command.to_string())
    }

    /// 返回执行命令用的 shell 配置。
    ///
    /// 返回:
    /// - 配置的 shell
    pub(crate) fn command_shell(&self) -> &str {
        &self.config.tools.command_shell
    }

    /// 返回工作区根目录。
    ///
    /// 返回:
    /// - 工作区路径
    pub(crate) fn workspace(&self) -> &Path {
        &self.workspace
    }
}
