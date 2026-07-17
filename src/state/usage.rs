use crate::llm::Usage;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Default, Serialize, Deserialize)]
struct UsageState {
    requests: u64,
    prompt_tokens: u64,
    completion_tokens: u64,
    total_tokens: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_usage: Option<Usage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_conversation_usage: Option<Usage>,
}

#[derive(Debug, Clone, Default)]
pub struct UsageSnapshot {
    pub requests: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    pub last_usage: Option<Usage>,
    pub last_conversation_usage: Option<Usage>,
}

impl From<UsageState> for UsageSnapshot {
    /// 将内部用量状态转换成只读快照。
    ///
    /// 参数:
    /// - `state`: 内部用量状态
    ///
    /// 返回:
    /// - 用量快照
    fn from(state: UsageState) -> Self {
        let last_conversation_usage = state
            .last_conversation_usage
            .clone()
            .or_else(|| state.last_usage.clone());
        Self {
            requests: state.requests,
            prompt_tokens: state.prompt_tokens,
            completion_tokens: state.completion_tokens,
            total_tokens: state.total_tokens,
            last_usage: state.last_usage,
            last_conversation_usage,
        }
    }
}

/// 累加主对话模型用量并保存最近一次主对话 provider usage。
///
/// 参数:
/// - `path`: 用量状态文件
/// - `usage`: 当前主对话请求 provider 返回的用量
///
/// 返回:
/// - 保存是否成功
pub fn add_usage(path: &Path, usage: &Usage) -> Result<()> {
    add_usage_with_scope(path, usage, true)
}

/// 累加辅助模型用量，不覆盖主对话最近一次 usage。
///
/// 参数:
/// - `path`: 用量状态文件
/// - `usage`: 当前辅助请求 provider 返回的用量
///
/// 返回:
/// - 保存是否成功
pub fn add_auxiliary_usage(path: &Path, usage: &Usage) -> Result<()> {
    add_usage_with_scope(path, usage, false)
}

/// 按请求类型累加模型用量。
///
/// 参数:
/// - `path`: 用量状态文件
/// - `usage`: 当前请求 provider 返回的用量
/// - `is_conversation`: 是否为用户可见主对话请求
///
/// 返回:
/// - 保存是否成功
fn add_usage_with_scope(path: &Path, usage: &Usage, is_conversation: bool) -> Result<()> {
    let mut state = if path.exists() {
        let raw = std::fs::read_to_string(path)?;
        serde_json::from_str(&raw).unwrap_or_default()
    } else {
        UsageState::default()
    };
    state.requests += 1;
    state.prompt_tokens += usage.prompt_tokens;
    state.completion_tokens += usage.completion_tokens;
    state.total_tokens += usage.total_tokens;
    state.last_usage = Some(usage.clone());
    if is_conversation {
        state.last_conversation_usage = Some(usage.clone());
    }
    std::fs::write(path, format!("{}\n", serde_json::to_string_pretty(&state)?))?;
    Ok(())
}

/// 读取最近一次 provider usage。
///
/// 参数:
/// - `path`: 用量状态文件
///
/// 返回:
/// - 最近一次 provider usage
#[cfg(test)]
pub fn last_usage(path: &Path) -> Result<Option<Usage>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path)?;
    let state = serde_json::from_str::<UsageState>(&raw).unwrap_or_default();
    Ok(state.last_usage)
}

/// 读取累计用量快照。
///
/// 参数:
/// - `path`: 用量状态文件
///
/// 返回:
/// - 累计用量快照
pub fn snapshot(path: &Path) -> Result<UsageSnapshot> {
    if !path.exists() {
        return Ok(UsageSnapshot::default());
    }
    let raw = std::fs::read_to_string(path)?;
    let state = serde_json::from_str::<UsageState>(&raw).unwrap_or_default();
    Ok(state.into())
}

/// 清空最近一次 provider usage。
///
/// 参数:
/// - `path`: 用量状态文件
///
/// 返回:
/// - 清空是否成功
pub fn clear_last_usage(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(path)?;
    let mut state = serde_json::from_str::<UsageState>(&raw).unwrap_or_default();
    state.last_usage = None;
    state.last_conversation_usage = None;
    std::fs::write(path, format!("{}\n", serde_json::to_string_pretty(&state)?))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_and_clears_last_usage() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("usage.json");
        let usage = Usage {
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
        };

        add_usage(&path, &usage).unwrap();
        assert_eq!(last_usage(&path).unwrap().unwrap().total_tokens, 15);
        assert_eq!(
            snapshot(&path)
                .unwrap()
                .last_conversation_usage
                .unwrap()
                .prompt_tokens,
            10
        );

        clear_last_usage(&path).unwrap();
        assert!(last_usage(&path).unwrap().is_none());
    }

    #[test]
    fn auxiliary_usage_does_not_replace_conversation_usage() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("usage.json");

        add_usage(
            &path,
            &Usage {
                prompt_tokens: 100,
                completion_tokens: 20,
                total_tokens: 120,
            },
        )
        .unwrap();
        add_auxiliary_usage(
            &path,
            &Usage {
                prompt_tokens: 5,
                completion_tokens: 2,
                total_tokens: 7,
            },
        )
        .unwrap();

        let snapshot = snapshot(&path).unwrap();
        assert_eq!(snapshot.total_tokens, 127);
        assert_eq!(snapshot.last_usage.unwrap().prompt_tokens, 5);
        assert_eq!(snapshot.last_conversation_usage.unwrap().prompt_tokens, 100);
    }
}
