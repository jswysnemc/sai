use super::availability::{backend_availability, BackendAvailability};
use super::policy::{FileAccess, SandboxPolicy};
use crate::i18n::text as t;
use serde::Serialize;

/// 沙箱状态快照，供 TUI `/sandbox` 与 Web 设置页共用。
#[derive(Debug, Clone, Serialize)]
pub(crate) struct SandboxStatus {
    /// 配置是否启用
    pub enabled: bool,
    /// 当前平台是否有沙箱实现
    pub platform_supported: bool,
    /// 后端探测结果
    #[serde(flatten)]
    pub backend: BackendAvailability,
    /// 网络策略
    pub network: &'static str,
    /// 是否清理密钥环境变量
    pub scrub_env: bool,
    /// 当前工作区解析出的可写目录
    pub writable_roots: Vec<String>,
    /// 当前工作区解析出的隐藏路径（仅列出存在的路径）
    pub deny_read: Vec<String>,
    /// 可写目录中保持只读的子路径（仅列出存在的路径）
    pub write_protected: Vec<String>,
    /// 当前环境中会被移除的变量名
    pub scrubbed_env: Vec<String>,
    /// 私有临时目录
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temp_dir: Option<String>,
}

/// 【沙箱】【状态汇总】按当前配置快照与工作区汇总沙箱状态。
///
/// 返回:
/// - 状态快照
pub(crate) fn sandbox_status() -> SandboxStatus {
    let settings = super::settings::current();
    let policy = SandboxPolicy::for_current_workspace(FileAccess::WorkspaceWrite, None).ok();
    let display = |paths: &[std::path::PathBuf], existing: bool| {
        paths
            .iter()
            .filter(|path| !existing || path.exists())
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
    };
    SandboxStatus {
        enabled: settings.config.enabled,
        platform_supported: super::settings::platform_supported(),
        backend: backend_availability(),
        network: settings.config.network.as_str(),
        scrub_env: settings.config.scrub_env,
        writable_roots: policy
            .as_ref()
            .map(|p| display(&p.writable_roots, false))
            .unwrap_or_default(),
        deny_read: policy
            .as_ref()
            .map(|p| display(&p.deny_read, true))
            .unwrap_or_default(),
        write_protected: policy
            .as_ref()
            .map(|p| display(&p.write_protected, true))
            .unwrap_or_default(),
        temp_dir: policy
            .as_ref()
            .and_then(|p| p.temp_dir.as_ref())
            .map(|dir| dir.display().to_string()),
        scrubbed_env: policy.map(|p| p.scrubbed_env).unwrap_or_default(),
    }
}

impl SandboxStatus {
    /// 返回一句话结论。
    ///
    /// 返回:
    /// - 生效、关闭、平台不支持或后端不可用
    pub(crate) fn headline(&self) -> String {
        if !self.platform_supported {
            return t(
                "Sandbox: not available on this platform; audited commands run after approval without isolation",
                "沙箱：当前平台不支持；审核模式的命令经批准后直接执行",
            )
            .to_string();
        }
        if !self.enabled {
            return t(
                "Sandbox: off (sandbox.enabled=false); audited commands run after approval without isolation",
                "沙箱：已关闭（sandbox.enabled=false）；审核模式的命令经批准后直接执行",
            )
            .to_string();
        }
        if !self.backend.available {
            return format!(
                "{} {}: {}",
                t("Sandbox: backend unavailable", "沙箱：后端不可用"),
                self.backend.backend,
                self.backend.reason.as_deref().unwrap_or_default()
            );
        }
        format!(
            "{} {} · {} {}",
            t("Sandbox: active via", "沙箱：已生效，后端"),
            self.backend.backend,
            t("network", "网络"),
            if self.network == "allow" {
                t("allowed", "允许")
            } else {
                t("blocked", "断开")
            }
        )
    }

    /// 生成 TUI 使用的多行说明。
    ///
    /// 返回:
    /// - 状态文本
    pub(crate) fn render_text(&self) -> String {
        let mut lines = vec![self.headline()];
        let list = |label: &str, items: &[String]| {
            if items.is_empty() {
                format!("  {label}: -")
            } else {
                format!("  {label}: {}", items.join(", "))
            }
        };
        lines.push(list(t("writable", "可写"), &self.writable_roots));
        lines.push(list(
            t("read-only inside writable", "可写目录中的只读路径"),
            &self.write_protected,
        ));
        lines.push(list(t("hidden", "隐藏"), &self.deny_read));
        lines.push(if self.scrub_env {
            list(t("scrubbed env", "已移除的环境变量"), &self.scrubbed_env)
        } else {
            format!("  {}", t("env scrubbing: off", "环境变量清理：关闭"))
        });
        lines.push(format!(
            "  {}",
            t(
                "Applies to run_command in audited, auto-audit and plan modes; edit with /config or config.jsonc sandbox.*",
                "作用于审核、自动审核与计划模式的 run_command；在 /config 或 config.jsonc 的 sandbox.* 中修改",
            )
        ));
        lines.join("\n")
    }
}
