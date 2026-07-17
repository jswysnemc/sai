use crate::state::turns::Turn;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct CompactionRequest {
    pub compact_turn_ids: Vec<String>,
    pub compact_turns: Vec<Turn>,
    pub previous_summary: Option<String>,
}

impl CompactionRequest {
    /// 创建会话压缩请求。
    ///
    /// 参数:
    /// - `compact_turns`: 需要压缩的旧轮次
    /// - `previous_summary`: 上一次压缩摘要
    ///
    /// 返回:
    /// - 会话压缩请求
    pub fn new(compact_turns: Vec<Turn>, previous_summary: Option<String>) -> Self {
        let compact_turn_ids = compact_turns
            .iter()
            .map(|turn| turn.turn_id.clone())
            .collect();
        Self {
            compact_turn_ids,
            compact_turns,
            previous_summary,
        }
    }

    /// 返回需要压缩的轮次数量。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 轮次数量
    pub fn turn_count(&self) -> usize {
        self.compact_turns.len()
    }

    /// 返回被压缩轮次 seq 范围。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 起止 seq，没有待压缩轮次时返回空
    pub(crate) fn seq_range(&self) -> Option<(i64, i64)> {
        let first = self.compact_turns.first()?;
        let last = self.compact_turns.last()?;
        Some((first.seq, last.seq))
    }

    /// 返回覆盖来源轮次数。
    ///
    /// 参数:
    /// - `previous_count`: 既有 checkpoint 覆盖轮次数
    ///
    /// 返回:
    /// - 新 checkpoint 应记录的累计覆盖轮次数
    pub(crate) fn source_turn_count_after_compaction(&self, previous_count: usize) -> usize {
        previous_count + self.turn_count()
    }

    /// 构造 checkpoint recent 上下文。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 最近被压缩轮次的可读文本
    pub(crate) fn recent_context(&self) -> String {
        self.compact_turns
            .iter()
            .rev()
            .take(2)
            .rev()
            .map(|turn| {
                format!(
                    "User: {}\nAssistant: {}",
                    turn.user_content, turn.assistant_content
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionSummary {
    pub updated_at: String,
    pub compacted_turns: usize,
    pub summary: String,
}
