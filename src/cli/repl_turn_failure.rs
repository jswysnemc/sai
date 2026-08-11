use crate::i18n::text as t;

/// 构造 TUI 中展示的轮次失败文本。
///
/// `anyhow::Error::to_string` 只返回最外层错误，工具执行失败的具体原因（缺失路径、
/// 权限不足等）都在错误链更深处。这里展开完整链路，用户不必再去翻日志确认失败点。
///
/// 参数:
/// - `error`: 轮次失败错误
///
/// 返回:
/// - 可直接交给 `record_meta` 的多行说明
pub(super) fn turn_failure_text(error: &anyhow::Error) -> String {
    let detail = crate::llm::error_detail_text(error);
    // 首行短标题供 Failure 渲染挂 `✗`；详情与重试提示放续行，避免整段详情顶掉引导符
    if crate::llm::is_transient_transport_error(error) {
        return format!(
            "{}\n{detail}\n{}",
            t("Connection interrupted", "连接中断"),
            t("You can retry this turn.", "可重试本轮请求。")
        );
    }
    format!(
        "{}\n{detail}\n{}",
        t("Turn failed", "本轮失败"),
        t(
            "The conversation is intact; you can adjust the request and send again.",
            "对话内容保持完整，可调整请求后重新发送。"
        )
    )
}

/// 构造 TUI 中展示的中断后失败文本。
///
/// 中断本身是用户主动行为，不需要"可重新发送"的引导，只补充失败原因。
///
/// 参数:
/// - `error`: 中断轮次携带的错误
///
/// 返回:
/// - 可直接交给 `record_meta` 的说明
pub(super) fn interrupted_failure_text(error: &anyhow::Error) -> String {
    format!(
        "{}: {}",
        t("Interrupted", "已中断"),
        crate::llm::error_detail_text(error)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证普通失败展示完整错误链。
    #[test]
    fn turn_failure_shows_full_error_chain() {
        let error = anyhow::anyhow!("read file failed, path does not exist: /tmp/a.txt")
            .context("tool read_file failed");

        let text = turn_failure_text(&error);

        assert!(text.contains("tool read_file failed"));
        assert!(text.contains("/tmp/a.txt"));
    }

    /// 验证断连错误保留可重试提示，且首行为短标题便于挂引导符。
    #[test]
    fn transient_failure_keeps_retry_hint() {
        let error = anyhow::anyhow!("error sending request for url: operation timed out")
            .context("client error (Connect)");

        let text = turn_failure_text(&error);
        let first = text.lines().next().unwrap_or("");

        assert!(
            first == "Connection interrupted" || first == "连接中断",
            "first={first:?}"
        );
        assert!(text.contains("error sending request"));
        assert!(text.contains("可重试") || text.contains("retry"));
    }

    /// 验证中断文本不含重新发送引导。
    #[test]
    fn interrupted_failure_omits_resend_hint() {
        let error = anyhow::anyhow!("stream closed");

        let text = interrupted_failure_text(&error);

        assert!(text.contains("stream closed"));
        assert!(!text.contains("重新发送"));
    }
}
