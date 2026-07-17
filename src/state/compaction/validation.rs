use anyhow::{bail, Result};

/// 计算压缩摘要允许占用的最大字符数。
///
/// 参数:
/// - `context_limit_chars`: 模型上下文字符预算
///
/// 返回:
/// - 摘要最大字符数
pub(crate) fn summary_char_limit(context_limit_chars: usize) -> usize {
    ((context_limit_chars as f32 * 0.15) as usize).clamp(512, 30_000)
}

/// 校验压缩摘要内容与体积。
///
/// 参数:
/// - `summary`: 模型生成的摘要
/// - `max_chars`: 摘要最大字符数
///
/// 返回:
/// - 摘要满足持久化约束时成功
pub(crate) fn validate_summary(summary: &str, max_chars: usize) -> Result<()> {
    let summary = summary.trim();
    if summary.is_empty() {
        bail!("compaction summary is empty")
    }
    let chars = summary.chars().count();
    if chars > max_chars {
        bail!("compaction summary exceeds size limit: summary_chars={chars}, max_chars={max_chars}")
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_summary() {
        assert!(validate_summary("   ", 10_000).is_err());
    }

    #[test]
    fn accepts_concise_markdown_without_fixed_headings() {
        assert!(validate_summary("## 进度\n- 已完成配置", 10_000).is_ok());
    }
}
