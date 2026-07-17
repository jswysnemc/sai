use super::model::{ContextSourceInput, ContextSourceInputState, ContextSourceSnapshot};
use super::snapshot::hash_text;
use anyhow::{bail, Result};
use std::collections::BTreeSet;

const SYSTEM_PROMPT_SOURCE: &str = "system_prompt";

/// 从稳定系统提示构造 Context Source 快照。
///
/// 参数:
/// - `system_prompt`: 当前稳定系统提示
///
/// 返回:
/// - 稳定排序后的 source 快照
pub(crate) fn stable_sources_from_prompt(system_prompt: &str) -> Vec<ContextSourceSnapshot> {
    stable_sources_from_inputs(&[ContextSourceInput::available(
        SYSTEM_PROMPT_SOURCE,
        system_prompt,
    )])
}

/// 构造稳定 baseline 文本。
///
/// 参数:
/// - `system_prompt`: 当前稳定系统提示
///
/// 返回:
/// - baseline 文本
pub(crate) fn stable_baseline(system_prompt: &str) -> String {
    system_prompt.to_string()
}

/// 从稳定系统提示构造 Context Source 输入。
///
/// 参数:
/// - `system_prompt`: 当前稳定系统提示
///
/// 返回:
/// - Context Source 输入
pub(crate) fn source_input_from_prompt(system_prompt: &str) -> ContextSourceInput {
    ContextSourceInput::available(SYSTEM_PROMPT_SOURCE, system_prompt)
}

/// 从输入中提取第一个不可用 source。
///
/// 参数:
/// - `inputs`: Context Source 输入集合
///
/// 返回:
/// - 格式化后的不可用 source
pub(crate) fn blocked_source(inputs: &[ContextSourceInput]) -> Option<String> {
    inputs.iter().find_map(|input| match &input.state {
        ContextSourceInputState::Blocked(reason) => Some(format!("{}: {}", input.key, reason)),
        ContextSourceInputState::Available(_) => None,
    })
}

/// 校验 Context Source 输入 key 唯一。
///
/// 参数:
/// - `inputs`: Context Source 输入集合
///
/// 返回:
/// - 校验是否通过
pub(crate) fn validate_unique_keys(inputs: &[ContextSourceInput]) -> Result<()> {
    let mut keys = BTreeSet::new();
    for input in inputs {
        if !keys.insert(input.key.as_str()) {
            bail!("duplicate Context Epoch source key: {}", input.key);
        }
    }
    Ok(())
}

/// 从输入构造稳定排序后的 source 快照。
///
/// 参数:
/// - `inputs`: Context Source 输入集合
///
/// 返回:
/// - 稳定排序后的 source 快照
pub(crate) fn stable_sources_from_inputs(
    inputs: &[ContextSourceInput],
) -> Vec<ContextSourceSnapshot> {
    let mut sources: Vec<ContextSourceSnapshot> = inputs
        .iter()
        .filter_map(|input| match &input.state {
            ContextSourceInputState::Available(text) => Some(ContextSourceSnapshot {
                key: input.key.clone(),
                text_hash: hash_text(text),
            }),
            ContextSourceInputState::Blocked(_) => None,
        })
        .collect();
    sources.sort_by(|left, right| left.key.cmp(&right.key));
    sources
}

/// 从输入构造稳定 baseline 文本。
///
/// 参数:
/// - `inputs`: Context Source 输入集合
///
/// 返回:
/// - baseline 文本
pub(crate) fn stable_baseline_from_inputs(inputs: &[ContextSourceInput]) -> String {
    let mut available: Vec<(&str, &str)> = inputs
        .iter()
        .filter_map(|input| match &input.state {
            ContextSourceInputState::Available(text) => Some((input.key.as_str(), text.as_str())),
            ContextSourceInputState::Blocked(_) => None,
        })
        .collect();
    available.sort_by(|left, right| left.0.cmp(right.0));
    if available.len() == 1 {
        return available[0].1.to_string();
    }
    available
        .into_iter()
        .map(|(key, text)| format!("<context-source key=\"{key}\">\n{text}\n</context-source>"))
        .collect::<Vec<String>>()
        .join("\n")
}
