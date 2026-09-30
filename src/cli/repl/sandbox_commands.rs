use super::*;

/// 【终端】【沙箱命令】处理 `/sandbox`，展示沙箱后端、策略与当前模式是否会隔离命令。
///
/// 参数:
/// - `input`: 提交文本
/// - `runtime`: 终端界面
/// - `mode`: 当前权限模式
///
/// 返回:
/// - 命中命令时返回 true
pub(super) fn handle(input: &str, runtime: &mut ReplRuntime, mode: AgentMode) -> Result<bool> {
    if !input.eq_ignore_ascii_case("/sandbox") {
        return Ok(false);
    }
    runtime.record_meta(status_text(mode))?;
    Ok(true)
}

/// 【终端】【启动面板】在权限模式后附加沙箱状态，YOLO 模式不经过沙箱因此不附加。
///
/// 参数:
/// - `mode`: 启动时的权限模式
///
/// 返回:
/// - 例如 `AUDIT · sandbox bwrap`
pub(super) fn welcome_mode_label(mode: AgentMode) -> String {
    if mode == AgentMode::Yolo {
        return mode.label().to_string();
    }
    format!(
        "{} · {}",
        mode.label(),
        sandbox_tag(&crate::sandbox::sandbox_status())
    )
}

/// 把沙箱状态压缩成启动面板中的短标签。
///
/// 参数:
/// - `status`: 沙箱状态
///
/// 返回:
/// - 短标签
fn sandbox_tag(status: &crate::sandbox::SandboxStatus) -> String {
    if !status.platform_supported || !status.enabled {
        return t("no sandbox", "无沙箱").to_string();
    }
    if !status.backend.available {
        return t("sandbox unavailable", "沙箱不可用").to_string();
    }
    format!("{} {}", t("sandbox", "沙箱"), status.backend.backend)
}

/// 生成 `/sandbox` 的展示文本。
///
/// 参数:
/// - `mode`: 当前权限模式
///
/// 返回:
/// - 状态说明与当前模式提示
fn status_text(mode: AgentMode) -> String {
    let status = crate::sandbox::sandbox_status();
    let scope = match mode {
        AgentMode::Yolo => t(
            "Current mode YOLO: commands run without the sandbox.",
            "当前为 YOLO 模式：命令不经过沙箱。",
        ),
        AgentMode::Plan => t(
            "Current mode Plan: inspection commands run in a read-only sandbox.",
            "当前为计划模式：检查命令在只读沙箱中执行。",
        ),
        AgentMode::Audited | AgentMode::AutoAudit => t(
            "Current mode audited: run_command uses the workspace sandbox unless an escalation is approved.",
            "当前为审核模式：run_command 使用工作区沙箱，批准提升的命令除外。",
        ),
    };
    format!("{}\n  {scope}", status.render_text())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证 YOLO 不附加沙箱标签，其余模式附加。
    #[test]
    fn welcome_label_appends_sandbox_state_outside_yolo() {
        assert_eq!(welcome_mode_label(AgentMode::Yolo), "YOLO");
        let audit = welcome_mode_label(AgentMode::Audited);
        assert!(audit.starts_with("AUDIT · "));
        assert!(audit.len() > "AUDIT · ".len());
    }
}
