use crate::state::turns::Turn;
use serde::{Deserialize, Serialize};

/// 运行中轮次的压缩范围。
///
/// 单个用户问题触发大量工具调用时，膨胀发生在轮次内部而非轮次之间。
/// 这里记录该轮次里有多少条工具调用应被摘要覆盖，重建上下文时跳过它们。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningTurnCompaction {
    /// 运行中轮次标识
    pub turn_id: String,
    /// 应被摘要覆盖的工具调用条数，从该轮次最早的调用开始计数
    pub compacted_calls: usize,
}

#[derive(Debug, Clone)]
pub struct CompactionRequest {
    pub compact_turn_ids: Vec<String>,
    pub compact_turns: Vec<Turn>,
    pub previous_summary: Option<String>,
    /// 本次同时压缩的运行中轮次范围
    pub running_turn: Option<RunningTurnCompaction>,
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
            running_turn: None,
        }
    }

    /// 附加运行中轮次的压缩范围。
    ///
    /// 参数:
    /// - `running_turn`: 运行中轮次范围；为空表示本次不压缩轮次内容
    ///
    /// 返回:
    /// - 携带运行中轮次范围的压缩请求
    pub fn with_running_turn(mut self, running_turn: Option<RunningTurnCompaction>) -> Self {
        self.running_turn = running_turn;
        self
    }

    /// 把运行中轮次的压缩边界推进到累计位置。
    ///
    /// selector 只看到"尚未压缩"的部分，写入 checkpoint 时必须换算成从该轮次
    /// 开头算起的累计条数，否则第二次压缩会把边界退回上一次的起点。
    ///
    /// 参数:
    /// - `offset`: 此前已被摘要覆盖的工具调用条数
    ///
    /// 返回:
    /// - 边界已累加的压缩请求
    pub fn with_compacted_call_offset(mut self, offset: usize) -> Self {
        if let Some(running) = self.running_turn.as_mut() {
            running.compacted_calls += offset;
        }
        self
    }

    /// 判断本次压缩是否有实际可压缩内容。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 存在待压缩轮次或运行中工具调用时返回 true
    pub fn has_content(&self) -> bool {
        !self.compact_turns.is_empty()
            || self
                .running_turn
                .as_ref()
                .is_some_and(|running| running.compacted_calls > 0)
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
