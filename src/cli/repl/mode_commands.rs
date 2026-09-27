use super::*;

/// 【终端】【权限模式】处理 `/plan`、`/audit`、`/yolo`、`/auto` 与 `/auto-audit`。
///
/// 参数: `input` 为提交文本，`mode` 为当前权限模式，`runtime` 为终端界面
/// 返回: 命中模式命令并完成切换时返回 true
pub(super) fn handle(input: &str, mode: &mut AgentMode, runtime: &mut ReplRuntime) -> Result<bool> {
    let Some(next) = parse(input) else {
        return Ok(false);
    };
    *mode = next;
    runtime.record_meta(format!("{}: {}", t("mode", "模式"), mode.label()))?;
    Ok(true)
}

/// 把模式命令映射到权限模式。
///
/// 参数:
/// - `input`: 已去除首尾空白的输入
///
/// 返回:
/// - 目标模式；非模式命令返回空
fn parse(input: &str) -> Option<AgentMode> {
    let input = input.to_ascii_lowercase();
    match input.as_str() {
        "/plan" => Some(AgentMode::Plan),
        "/audit" => Some(AgentMode::Audited),
        "/yolo" => Some(AgentMode::Yolo),
        "/auto" | "/auto-audit" => Some(AgentMode::AutoAudit),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_every_mode_command() {
        assert_eq!(parse("/PLAN"), Some(AgentMode::Plan));
        assert_eq!(parse("/audit"), Some(AgentMode::Audited));
        assert_eq!(parse("/yolo"), Some(AgentMode::Yolo));
        assert_eq!(parse("/auto-audit"), Some(AgentMode::AutoAudit));
        assert_eq!(parse("/auto now"), None);
    }
}
