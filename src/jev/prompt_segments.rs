//! 【Jev路由】【提示词解析】分离静态文字与按需片段，不依赖会话或工具状态。
use anyhow::{bail, Context, Result};

/// 一段等待路由判断的完整提示词。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PromptSegment {
    pub description: String,
    pub content: String,
}

/// 分离后的静态提示和候选片段。
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct ParsedPrompt {
    pub baseline: String,
    pub segments: Vec<PromptSegment>,
}

/// 【Jev路由】【提示词解析】解析标签；参数为原始提示，返回静态文字与候选，格式错误时报错。
pub(crate) fn parse(source: &str) -> Result<ParsedPrompt> {
    let mut parsed = ParsedPrompt::default();
    let mut remaining = source;
    while let Some(start) = next_open(remaining) {
        let prefix = &remaining[..start];
        if prefix.contains("</jev>") {
            bail!("unmatched </jev> in prompt");
        }
        parsed.baseline.push_str(prefix);
        let tagged = &remaining[start + 4..];
        let (description, body_start) = opening(tagged)?;
        let body = &tagged[body_start..];
        let end = body.find("</jev>").context("unclosed <jev> in prompt")?;
        let content = &body[..end];
        if next_open(content).is_some() {
            bail!("nested <jev> tags are not supported");
        }
        if !content.trim().is_empty() {
            parsed.segments.push(PromptSegment {
                description: description.unwrap_or_else(|| content.trim().to_string()),
                content: content.trim().to_string(),
            });
        }
        remaining = &body[end + 6..];
    }
    if remaining.contains("</jev>") {
        bail!("unmatched </jev> in prompt");
    }
    parsed.baseline.push_str(remaining);
    Ok(parsed)
}

/// 查找精确的开始标签；参数为剩余文本，返回字节位置，忽略相似标签名。
fn next_open(source: &str) -> Option<usize> {
    source.match_indices("<jev").find_map(|(index, _)| {
        let after = &source[index + 4..];
        (after.is_empty() || after.starts_with('>') || after.starts_with(char::is_whitespace))
            .then_some(index)
    })
}

/// 解析开始标签的可选描述；参数从标签名之后开始，返回描述和正文偏移。
fn opening(source: &str) -> Result<(Option<String>, usize)> {
    let mut tail = source.trim_start();
    if let Some(rest) = tail.strip_prefix('>') {
        return Ok((None, source.len() - rest.len()));
    }
    tail = tail
        .strip_prefix("description")
        .context("<jev> only accepts description")?
        .trim_start();
    tail = tail
        .strip_prefix('=')
        .context("expected = after jev description")?
        .trim_start();
    let quote = tail
        .chars()
        .next()
        .context("missing jev description value")?;
    if quote != '"' && quote != '\'' {
        bail!("jev description must be quoted");
    }
    tail = &tail[1..];
    let end = tail.find(quote).context("unclosed jev description quote")?;
    let description = tail[..end].trim().to_string();
    tail = tail[end + 1..].trim_start();
    let rest = tail
        .strip_prefix('>')
        .context("unexpected attribute in <jev>")?;
    Ok((
        (!description.is_empty()).then_some(description),
        source.len() - rest.len(),
    ))
}

/// 按路由开关过滤静态内容；参数为原文与开关，关闭时逐字返回原文。
pub(crate) fn baseline(source: &str, routing: bool) -> Result<String> {
    if routing {
        Ok(parse(source)?.baseline)
    } else {
        Ok(source.to_string())
    }
}

#[cfg(test)]
#[path = "prompt_segments_tests.rs"]
mod tests;
