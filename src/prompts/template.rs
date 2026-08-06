use crate::config::PromptTemplateConfig;
use anyhow::{bail, Result};
use std::collections::BTreeMap;

/// 完成变量替换后的系统提示词和用户提示词。
#[derive(Debug)]
pub(crate) struct RenderedPrompt {
    pub system: String,
    pub user: String,
}

impl RenderedPrompt {
    /// 返回系统提示词与用户提示词的总字符数。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 两段提示词的 Unicode 字符数之和
    pub(crate) fn total_chars(&self) -> usize {
        self.system.chars().count() + self.user.chars().count()
    }
}

/// 解析提示词中的 `{{variable}}` 占位符。
///
/// 变量值只替换一次，值本身包含的花括号不会再次进入解析，避免仓库 diff、
/// 用户消息或历史内容意外成为新的模板表达式。
///
/// 参数:
/// - `template`: 待解析的模板
/// - `variables`: 变量名称和值
///
/// 返回:
/// - 完成变量替换的提示词
pub(crate) fn render_template(template: &str, variables: &[(&str, &str)]) -> Result<String> {
    let values = variables.iter().copied().collect::<BTreeMap<_, _>>();
    let occurrences = parse_occurrences(template)?;
    let mut rendered = String::with_capacity(template.len());
    let mut cursor = 0;
    for occurrence in occurrences {
        rendered.push_str(&template[cursor..occurrence.start]);
        let Some(value) = values.get(occurrence.name.as_str()) else {
            bail!("unknown prompt template variable: {}", occurrence.name);
        };
        rendered.push_str(value);
        cursor = occurrence.end;
    }
    rendered.push_str(&template[cursor..]);
    Ok(rendered)
}

/// 同时解析一组系统提示词和用户提示词。
///
/// 参数:
/// - `template`: 系统与用户提示词配置
/// - `variables`: 变量名称和值
///
/// 返回:
/// - 完成变量替换的两段提示词
pub(crate) fn render_prompt_pair(
    template: &PromptTemplateConfig,
    variables: &[(&str, &str)],
) -> Result<RenderedPrompt> {
    Ok(RenderedPrompt {
        system: render_template(&template.system, variables)?,
        user: render_template(&template.user, variables)?,
    })
}

/// 校验单段模板只使用允许变量，并包含全部必要变量。
///
/// 参数:
/// - `template`: 待校验模板
/// - `allowed`: 允许使用的变量名
/// - `required`: 必须且只能出现一次的变量名
///
/// 返回:
/// - 模板合法时返回空结果
#[cfg(test)]
pub(crate) fn validate_template(template: &str, allowed: &[&str], required: &[&str]) -> Result<()> {
    validate_occurrences(&parse_occurrences(template)?, allowed, required)
}

/// 校验系统提示词与用户提示词组成的完整模板。
///
/// 必要变量可以放在任一段中，但总计必须出现一次。
///
/// 参数:
/// - `template`: 系统与用户提示词配置
/// - `allowed`: 允许使用的变量名
/// - `required`: 必须且只能出现一次的变量名
///
/// 返回:
/// - 模板合法时返回空结果
pub(crate) fn validate_prompt_pair(
    template: &PromptTemplateConfig,
    allowed: &[&str],
    required: &[&str],
) -> Result<()> {
    if template.system.trim().is_empty() {
        bail!("prompt template system text cannot be empty");
    }
    if template.user.trim().is_empty() {
        bail!("prompt template user text cannot be empty");
    }
    let mut occurrences = parse_occurrences(&template.system)?;
    occurrences.extend(parse_occurrences(&template.user)?);
    validate_occurrences(&occurrences, allowed, required)
}

struct VariableOccurrence {
    name: String,
    start: usize,
    end: usize,
}

/// 扫描模板变量并保留其字节范围。
///
/// 参数:
/// - `template`: 待扫描模板
///
/// 返回:
/// - 按出现顺序排列的变量
fn parse_occurrences(template: &str) -> Result<Vec<VariableOccurrence>> {
    let mut occurrences = Vec::new();
    let mut cursor = 0;
    while let Some(relative_start) = template[cursor..].find("{{") {
        let start = cursor + relative_start;
        if template[cursor..start].contains("}}") {
            bail!("unexpected closing prompt template delimiter");
        }
        let content_start = start + 2;
        let Some(relative_end) = template[content_start..].find("}}") else {
            bail!("unclosed prompt template variable");
        };
        let close_start = content_start + relative_end;
        let end = close_start + 2;
        let name = template[content_start..close_start].trim();
        if name.is_empty() || name.contains(['{', '}']) {
            bail!("invalid prompt template variable: {name}");
        }
        occurrences.push(VariableOccurrence {
            name: name.to_string(),
            start,
            end,
        });
        cursor = end;
    }
    if template[cursor..].contains("}}") {
        bail!("unexpected closing prompt template delimiter");
    }
    Ok(occurrences)
}

/// 校验已经扫描的变量集合。
///
/// 参数:
/// - `occurrences`: 模板中的变量
/// - `allowed`: 允许变量
/// - `required`: 必要变量
///
/// 返回:
/// - 变量集合合法时返回空结果
fn validate_occurrences(
    occurrences: &[VariableOccurrence],
    allowed: &[&str],
    required: &[&str],
) -> Result<()> {
    let mut counts = BTreeMap::<&str, usize>::new();
    for occurrence in occurrences {
        if !allowed.contains(&occurrence.name.as_str()) {
            bail!("unknown prompt template variable: {}", occurrence.name);
        }
        *counts.entry(occurrence.name.as_str()).or_default() += 1;
    }
    for name in required {
        match counts.get(name).copied().unwrap_or_default() {
            1 => {}
            0 => bail!("required prompt template variable is missing: {name}"),
            count => bail!("prompt template variable must appear once: {name} ({count})"),
        }
    }
    Ok(())
}
