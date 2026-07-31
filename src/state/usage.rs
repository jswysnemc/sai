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
        Self {
            requests: state.requests,
            prompt_tokens: state.prompt_tokens,
            completion_tokens: state.completion_tokens,
            total_tokens: state.total_tokens,
            last_usage: state.last_usage,
            last_conversation_usage: state.last_conversation_usage,
        }
    }
}

/// 累加一个完整轮次的用量，并单独记录上下文占用口径。
///
/// 一个轮次可能包含多次模型调用，累计量与上下文占用量必须分开：
/// 前者决定统计总量，后者决定上下文进度条。
///
/// 参数:
/// - `path`: 用量状态文件
/// - `turn_usage`: 本轮全部模型调用的用量累计
/// - `context_usage`: 最后一次调用的用量；缺失时清空 provider 上下文口径
///
/// 返回:
/// - 保存是否成功
pub fn add_turn_usage(
    path: &Path,
    turn_usage: &Usage,
    context_usage: Option<&Usage>,
) -> Result<()> {
    let mut state = load_state(path)?;
    // 1. 总量按整轮累计，工具轮次不再漏计
    state.requests += 1;
    state.prompt_tokens += turn_usage.prompt_tokens;
    state.completion_tokens += turn_usage.completion_tokens;
    state.total_tokens += turn_usage.total_tokens;
    // 2. 最近用量保留整轮统计；上下文口径只接受最后一次 provider 明确上报
    state.last_usage = Some(turn_usage.clone());
    state.last_conversation_usage = context_usage.cloned();
    save_state(path, &state)
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
    let mut state = load_state(path)?;
    state.requests += 1;
    state.prompt_tokens += usage.prompt_tokens;
    state.completion_tokens += usage.completion_tokens;
    state.total_tokens += usage.total_tokens;
    // 辅助调用不覆盖主对话的上下文口径，否则上下文进度条会被压缩摘要之类的小请求带偏
    state.last_usage = Some(usage.clone());
    save_state(path, &state)
}

/// 读取用量状态文件。
///
/// 参数:
/// - `path`: 用量状态文件
///
/// 返回:
/// - 已有状态；文件缺失或解析失败时返回默认值
fn load_state(path: &Path) -> Result<UsageState> {
    if !path.exists() {
        return Ok(UsageState::default());
    }
    let raw = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&raw).unwrap_or_default())
}

/// 写回用量状态文件。
///
/// 参数:
/// - `path`: 用量状态文件
/// - `state`: 待写入的状态
///
/// 返回:
/// - 写入是否成功
fn save_state(path: &Path, state: &UsageState) -> Result<()> {
    std::fs::write(path, format!("{}\n", serde_json::to_string_pretty(state)?))?;
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

/// 清空最近一次主对话 provider usage，保留累计量与最近一次已上报用量。
///
/// 参数:
/// - `path`: 用量状态文件
///
/// 返回:
/// - 清空是否成功
pub fn clear_last_conversation_usage(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(path)?;
    let mut state = serde_json::from_str::<UsageState>(&raw).unwrap_or_default();
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
            ..Usage::default()
        };

        add_turn_usage(&path, &usage, Some(&usage)).unwrap();
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

        let conversation = Usage {
            prompt_tokens: 100,
            completion_tokens: 20,
            total_tokens: 120,
            ..Usage::default()
        };
        add_turn_usage(&path, &conversation, Some(&conversation)).unwrap();
        add_auxiliary_usage(
            &path,
            &Usage {
                prompt_tokens: 5,
                completion_tokens: 2,
                total_tokens: 7,
                ..Usage::default()
            },
        )
        .unwrap();

        let snapshot = snapshot(&path).unwrap();
        assert_eq!(snapshot.total_tokens, 127);
        assert_eq!(snapshot.last_usage.unwrap().prompt_tokens, 5);
        assert_eq!(snapshot.last_conversation_usage.unwrap().prompt_tokens, 100);
    }

    /// 验证整轮累计计入总量，而上下文口径只取最后一次调用。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无；断言失败则测试不通过
    #[test]
    fn turn_total_and_context_usage_stay_separate() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("usage.json");
        // 三轮工具调用累计 30 万输入，最后一次调用的上下文占用为 12 万
        let turn = Usage {
            prompt_tokens: 300_000,
            completion_tokens: 3_000,
            total_tokens: 303_000,
            cache_read_tokens: 280_000,
            cache_write_tokens: 0,
        };
        let context = Usage {
            prompt_tokens: 120_000,
            completion_tokens: 900,
            total_tokens: 120_900,
            cache_read_tokens: 115_000,
            cache_write_tokens: 0,
        };
        add_turn_usage(&path, &turn, Some(&context)).unwrap();

        let snapshot = snapshot(&path).unwrap();
        assert_eq!(snapshot.prompt_tokens, 300_000);
        assert_eq!(snapshot.total_tokens, 303_000);
        assert_eq!(
            snapshot.last_conversation_usage.unwrap().prompt_tokens,
            120_000
        );
    }

    /// 【状态】【上下文用量】验证最后一次调用未上报 usage 时不使用整轮累计量。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn missing_context_usage_does_not_reuse_turn_total() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("usage.json");
        let turn = Usage {
            prompt_tokens: 300_000,
            completion_tokens: 3_000,
            total_tokens: 303_000,
            cache_read_tokens: 280_000,
            cache_write_tokens: 0,
        };

        add_turn_usage(&path, &turn, None).unwrap();

        let snapshot = snapshot(&path).unwrap();
        assert_eq!(snapshot.last_usage.unwrap().prompt_tokens, 300_000);
        assert!(snapshot.last_conversation_usage.is_none());
    }

    /// 【状态】【上下文用量】验证整轮无 usage 时只清空上下文口径。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn clearing_conversation_usage_keeps_last_reported_total() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("usage.json");
        let usage = Usage {
            prompt_tokens: 120_000,
            completion_tokens: 1_000,
            total_tokens: 121_000,
            cache_read_tokens: 100_000,
            cache_write_tokens: 0,
        };
        add_turn_usage(&path, &usage, Some(&usage)).unwrap();

        clear_last_conversation_usage(&path).unwrap();

        let snapshot = snapshot(&path).unwrap();
        assert_eq!(snapshot.last_usage.unwrap().prompt_tokens, 120_000);
        assert!(snapshot.last_conversation_usage.is_none());
    }
}
