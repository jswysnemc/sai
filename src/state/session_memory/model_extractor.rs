use super::extractor::{clip_text, estimate_turn_chars, is_disabled, record_extraction_failure};
use super::model::{NewSessionMemory, SessionMemory};
use super::repository::{load_memory, upsert_memory};
use crate::state::turns::{ConversationDb, Turn, TurnStatus};
use crate::state::StateStore;
use anyhow::Result;

/// 模型提取 Session Memory 的预算配置。
#[derive(Debug, Clone)]
pub(crate) struct ModelExtractionConfig {
    pub max_prompt_chars: usize,
    pub max_source_chars: usize,
    pub max_summary_chars: usize,
    pub failure_threshold: usize,
    pub disable_seconds: i64,
}

impl ModelExtractionConfig {
    /// 按当前模型上下文窗口生成独立提取预算。
    ///
    /// 参数:
    /// - `context_limit_chars`: 当前主对话模型上下文窗口字符数
    ///
    /// 返回:
    /// - 模型提取预算配置
    pub(crate) fn for_context_limit(context_limit_chars: usize) -> Self {
        let base_budget = if context_limit_chars == 0 {
            6_000
        } else {
            ((context_limit_chars as f32) * 0.2) as usize
        };
        let max_prompt_chars = base_budget.clamp(2_000, 24_000);
        Self {
            max_prompt_chars,
            max_source_chars: max_prompt_chars.saturating_sub(1_200).max(800),
            max_summary_chars: (max_prompt_chars / 3).clamp(800, 8_000),
            failure_threshold: 3,
            disable_seconds: 3_600,
        }
    }
}

/// 模型提取 Session Memory 的输入。
#[derive(Debug, Clone)]
pub(crate) struct ModelExtractionInput {
    pub prompt: String,
    #[allow(dead_code)]
    pub prompt_chars: usize,
    config: ModelExtractionConfig,
    source_turns: Vec<Turn>,
    total_source_turn_count: usize,
}

/// 准备模型提取 Session Memory 的独立输入。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `config`: 模型提取预算配置
///
/// 返回:
/// - 模型提取输入，当前无可提取内容或熔断时返回空
pub(crate) fn prepare_model_extraction_input(
    db: &ConversationDb,
    session_id: &str,
    config: &ModelExtractionConfig,
) -> Result<Option<ModelExtractionInput>> {
    let previous = load_memory(db, session_id)?;
    if is_disabled(previous.as_ref()) {
        return Ok(None);
    }
    let completed_turns = db
        .load_turns_after_seq(0, None)?
        .into_iter()
        .filter(|turn| turn.status != TurnStatus::Running)
        .collect::<Vec<_>>();
    if completed_turns.is_empty() {
        return Ok(None);
    }
    let source_turns = select_source_turns(&completed_turns, config.max_source_chars);
    let prompt = model_extraction_prompt(&source_turns, previous.as_ref(), config);
    Ok(Some(ModelExtractionInput {
        prompt_chars: prompt.chars().count(),
        prompt,
        config: config.clone(),
        source_turns,
        total_source_turn_count: completed_turns.len(),
    }))
}

impl StateStore {
    /// 准备当前会话的模型 Session Memory 提取输入。
    ///
    /// 参数:
    /// - `context_limit_chars`: 当前主模型上下文窗口字符数
    ///
    /// 返回:
    /// - 模型提取输入
    pub(crate) fn prepare_session_memory_model_extraction(
        &self,
        context_limit_chars: usize,
    ) -> Result<Option<ModelExtractionInput>> {
        let config = ModelExtractionConfig::for_context_limit(context_limit_chars);
        prepare_model_extraction_input(&self.conv_db, &self.session_id, &config)
    }

    /// 应用当前会话的模型 Session Memory 提取摘要。
    ///
    /// 参数:
    /// - `input`: 模型提取输入
    /// - `summary`: 模型生成摘要
    ///
    /// 返回:
    /// - 写入是否成功
    pub(crate) fn apply_session_memory_model_extraction(
        &self,
        input: &ModelExtractionInput,
        summary: &str,
    ) -> Result<()> {
        apply_model_extraction_summary(
            &self.conv_db,
            &self.session_id,
            input,
            summary,
            &input.config,
        )
    }

    /// 记录当前会话的模型 Session Memory 提取失败。
    ///
    /// 参数:
    /// - `input`: 模型提取输入
    /// - `reason`: 失败原因
    ///
    /// 返回:
    /// - 记录是否成功
    pub(crate) fn record_session_memory_model_extraction_failure(
        &self,
        input: &ModelExtractionInput,
        reason: &str,
    ) -> Result<()> {
        record_model_extraction_failure(
            &self.conv_db,
            &self.session_id,
            reason,
            input,
            &input.config,
        )
    }
}

/// 应用模型提取出的 Session Memory 摘要。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `input`: 模型提取输入
/// - `summary`: 模型生成摘要
/// - `config`: 模型提取预算配置
///
/// 返回:
/// - 写入是否成功
pub(crate) fn apply_model_extraction_summary(
    db: &ConversationDb,
    session_id: &str,
    input: &ModelExtractionInput,
    summary: &str,
    config: &ModelExtractionConfig,
) -> Result<()> {
    let summary = summary.trim();
    if summary.is_empty() {
        record_model_extraction_failure(
            db,
            session_id,
            "session memory model summary is empty",
            input,
            config,
        )?;
        anyhow::bail!("session memory model summary is empty");
    }
    if summary.chars().count() > config.max_summary_chars {
        record_model_extraction_failure(
            db,
            session_id,
            "session memory model summary exceeds budget",
            input,
            config,
        )?;
        anyhow::bail!("session memory model summary exceeds budget");
    }
    let last_turn = input
        .source_turns
        .last()
        .ok_or_else(|| anyhow::anyhow!("session memory model extraction has no source turns"))?;
    upsert_memory(
        db,
        NewSessionMemory {
            session_id: session_id.to_string(),
            summary: summary.to_string(),
            last_summarized_turn_id: Some(last_turn.turn_id.clone()),
            last_summarized_seq: last_turn.seq,
            checkpoint_id: crate::state::failure_recovery::latest_checkpoint_id(db)?,
            source_turn_count: input.total_source_turn_count,
            token_estimate: crate::token_estimate::estimate_tokens(summary),
        },
    )?;
    Ok(())
}

/// 记录模型提取 Session Memory 失败。
///
/// 参数:
/// - `db`: 对话数据库
/// - `session_id`: 会话标识
/// - `reason`: 失败原因
/// - `input`: 模型提取输入
/// - `config`: 模型提取预算配置
///
/// 返回:
/// - 记录是否成功
pub(crate) fn record_model_extraction_failure(
    db: &ConversationDb,
    session_id: &str,
    reason: &str,
    input: &ModelExtractionInput,
    config: &ModelExtractionConfig,
) -> Result<()> {
    record_extraction_failure(
        db,
        session_id,
        reason,
        &input.source_turns,
        config.failure_threshold,
        config.disable_seconds,
    )
}

/// 选择模型提取的来源轮次。
///
/// 参数:
/// - `turns`: 当前非运行轮次
/// - `max_source_chars`: 来源内容预算
///
/// 返回:
/// - 参与模型提取的来源轮次
fn select_source_turns(turns: &[Turn], max_source_chars: usize) -> Vec<Turn> {
    let mut selected = Vec::new();
    let mut total_chars = 0usize;
    for turn in turns.iter().rev() {
        let turn_chars = estimate_turn_chars(std::slice::from_ref(turn));
        if !selected.is_empty() && total_chars.saturating_add(turn_chars) > max_source_chars {
            break;
        }
        total_chars = total_chars.saturating_add(turn_chars);
        selected.push(turn.clone());
    }
    selected.reverse();
    selected
}

/// 构造模型提取 Session Memory 的提示词。
///
/// 参数:
/// - `turns`: 来源轮次
/// - `previous`: 既有会话工作记忆
/// - `config`: 模型提取预算配置
///
/// 返回:
/// - 提示词
fn model_extraction_prompt(
    turns: &[Turn],
    previous: Option<&SessionMemory>,
    config: &ModelExtractionConfig,
) -> String {
    let mut lines = Vec::new();
    lines.push("Update the session memory for future turns.".to_string());
    lines.push("Keep stable facts, user intent, constraints, decisions, and next actions. Remove transient chatter.".to_string());
    lines.push(format!(
        "Return plain Markdown within {} characters.",
        config.max_summary_chars
    ));
    if let Some(memory) = previous.filter(|memory| !memory.summary.trim().is_empty()) {
        lines.push("Existing memory:".to_string());
        lines.push(clip_text(
            &memory.summary,
            (config.max_prompt_chars / 4).max(500),
        ));
    }
    lines.push("Recent source turns:".to_string());
    let per_turn_budget = (config.max_source_chars / turns.len().max(1)).clamp(300, 2_000);
    for turn in turns {
        lines.push(format!(
            "seq {} user:\n{}",
            turn.seq,
            clip_text(&turn.user_content, per_turn_budget)
        ));
        lines.push(format!(
            "seq {} assistant:\n{}",
            turn.seq,
            clip_text(&turn.assistant_content, per_turn_budget)
        ));
    }
    clip_text(&lines.join("\n\n"), config.max_prompt_chars)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::session_memory::repository::load_memory;
    use tempfile::TempDir;

    fn test_db() -> (TempDir, ConversationDb) {
        let temp = tempfile::tempdir().unwrap();
        let db = ConversationDb::open(temp.path()).unwrap();
        (temp, db)
    }

    #[test]
    fn model_extraction_input_respects_prompt_budget() {
        let (_temp, db) = test_db();
        db.start_turn("turn_1", &"user ".repeat(1_000)).unwrap();
        db.complete_turn("turn_1", &"assistant ".repeat(1_000), None)
            .unwrap();
        let config = ModelExtractionConfig {
            max_prompt_chars: 1_200,
            max_source_chars: 900,
            max_summary_chars: 400,
            failure_threshold: 3,
            disable_seconds: 3_600,
        };

        let input = prepare_model_extraction_input(&db, "default", &config)
            .unwrap()
            .expect("model extraction input");

        assert!(input.prompt_chars <= config.max_prompt_chars);
        assert!(input.prompt.contains("Recent source turns"));
    }

    #[test]
    fn model_extraction_summary_updates_memory() {
        let (_temp, db) = test_db();
        db.start_turn("turn_1", "new task").unwrap();
        db.complete_turn("turn_1", "new answer", None).unwrap();
        let config = ModelExtractionConfig::for_context_limit(10_000);
        let input = prepare_model_extraction_input(&db, "default", &config)
            .unwrap()
            .expect("model extraction input");

        apply_model_extraction_summary(&db, "default", &input, "model memory summary", &config)
            .unwrap();

        let memory = load_memory(&db, "default").unwrap().unwrap();
        assert_eq!(memory.summary, "model memory summary");
        assert_eq!(memory.last_summarized_seq, 1);
        assert_eq!(memory.source_turn_count, 1);
        assert_eq!(memory.consecutive_failures, 0);
    }
}
