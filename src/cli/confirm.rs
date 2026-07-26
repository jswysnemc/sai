use super::*;

/// 请求用户确认一次不可恢复的破坏性操作。
///
/// 参数:
/// - `action`: 操作描述，用于提示语
/// - `assume_yes`: 携带 --yes 时跳过确认
///
/// 返回:
/// - 用户是否确认；非交互环境且未携带 --yes 时返回错误
pub(super) fn confirm_destructive(action: &str, assume_yes: bool) -> Result<bool> {
    if assume_yes {
        return Ok(true);
    }
    // 1. 非交互环境不能静默执行破坏性操作，要求显式 --yes
    if !(io::stdin().is_terminal() && io::stdout().is_terminal()) {
        bail!(
            "{}",
            if crate::i18n::is_zh() {
                format!("{action} 不可恢复；非交互环境请携带 --yes 执行")
            } else {
                format!("{action} is irreversible; pass --yes in non-interactive environments")
            }
        );
    }
    // 2. 提示写 stderr，避免污染可解析输出
    eprint!(
        "{}",
        if crate::i18n::is_zh() {
            format!("{action}，此操作不可恢复。确认继续？[y/N] ")
        } else {
            format!("{action}; this cannot be undone. Continue? [y/N] ")
        }
    );
    io::stderr().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    let confirmed = matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes");
    if !confirmed {
        eprintln!("{}", t("cancelled", "已取消"));
    }
    Ok(confirmed)
}
