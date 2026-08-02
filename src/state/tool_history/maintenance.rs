use super::model::{NewToolOutputReplacement, ToolExchangeRecord};
use super::repository::{load_tool_exchanges_for_turn, upsert_tool_output_replacement};
use crate::state::compaction::PRESERVED_RECENT_TURNS;
use crate::state::turns::TurnStatus;
use crate::state::StateStore;
use anyhow::Result;

/// 陈旧工具结果被裁剪后的 replacement 策略标识。
pub(crate) const POLICY_STALE_SNIP: &str = "stale_snip";

/// 陈旧工具结果被折叠后的 replacement 策略标识。
pub(crate) const POLICY_STALE_PRUNE: &str = "stale_prune";

/// 触发维护的最小可见字符数，低于该体积的结果改写收益抵不过标记开销。
const MIN_MAINTAIN_CHARS: usize = 1_024;

/// 陈旧工具结果维护模式。
///
/// 这是上下文管理的免费半区：陈旧工具结果可以重新获取（文件可以重读、
/// 命令可以重跑），改写它们既不调用摘要模型也不丢消息，工具调用与结果的
/// 配对关系保持不变。付费的摘要压缩只在这层不够用时才发生。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolResultMaintenanceMode {
    /// 裁剪：保留首尾片段，中段省略
    Snip,
    /// 折叠：整体替换为一行占位说明，可对已裁剪结果二次升级
    Prune,
}

/// 一次维护扫描的结果统计。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ToolResultMaintenanceStats {
    /// 被改写的工具结果数量
    pub rewritten: usize,
    /// 改写节省的可见字符数
    pub saved_chars: usize,
}

/// 首尾裁剪几何参数。
struct SnipGeometry {
    head_lines: usize,
    tail_lines: usize,
    head_chars: usize,
    tail_chars: usize,
}

/// 输出两端都可能携带关键信息的工具：命令的失败可能在头也可能在尾。
const BALANCED_SNIP_TOOLS: &[&str] = &["run_command", "background_command", "subagent"];

impl StateStore {
    /// 改写陈旧工具结果，覆盖已完成轮次与运行中轮次的较早部分。
    ///
    /// 与压缩选择器保持同一口径：已完成轮次全部参与，运行中轮次保留末尾若干条
    /// 最新结果。单个用户问题触发大量工具调用时膨胀发生在轮次内部，
    /// 只维护已完成轮次等于不维护。
    /// 幂等：已折叠的结果不再处理，已裁剪的结果只会被折叠模式升级。
    /// 错误结果保持原文，模型需要完整错误现场来避免重蹈覆辙。
    ///
    /// 参数:
    /// - `mode`: 裁剪或折叠
    ///
    /// 返回:
    /// - 改写数量与节省字符数统计
    pub fn maintain_stale_tool_results(
        &self,
        mode: ToolResultMaintenanceMode,
    ) -> Result<ToolResultMaintenanceStats> {
        let mut stats = ToolResultMaintenanceStats::default();
        let turns = self.conv_db.load_turns()?;
        let non_running = turns
            .iter()
            .filter(|turn| turn.status != TurnStatus::Running)
            .cloned()
            .collect::<Vec<_>>();
        let stale_len = non_running.len().saturating_sub(PRESERVED_RECENT_TURNS);
        // 1. 已完成轮次的工具结果全部参与维护
        for turn in &non_running[..stale_len] {
            let exchanges =
                load_tool_exchanges_for_turn(&self.conv_db, &self.session_id, &turn.turn_id)?;
            for exchange in &exchanges {
                self.maintain_exchange(mode, exchange, &mut stats)?;
            }
        }
        // 2. 运行中轮次同样参与，但保留末尾最新的若干条供模型继续使用
        if let Some(running) = turns
            .iter()
            .find(|turn| turn.status == TurnStatus::Running)
        {
            let exchanges =
                load_tool_exchanges_for_turn(&self.conv_db, &self.session_id, &running.turn_id)?;
            let stale_len = exchanges
                .len()
                .saturating_sub(crate::state::compaction::PRESERVED_RUNNING_TOOL_CALLS);
            for exchange in &exchanges[..stale_len] {
                self.maintain_exchange(mode, exchange, &mut stats)?;
            }
        }
        Ok(stats)
    }

    /// 按模式处理单条工具交换记录。
    ///
    /// 参数:
    /// - `mode`: 裁剪或折叠
    /// - `exchange`: 工具调用与结果记录
    /// - `stats`: 统计累加器
    ///
    /// 返回:
    /// - 写入是否成功
    fn maintain_exchange(
        &self,
        mode: ToolResultMaintenanceMode,
        exchange: &ToolExchangeRecord,
        stats: &mut ToolResultMaintenanceStats,
    ) -> Result<()> {
        let Some(result) = &exchange.result else {
            return Ok(());
        };
        if !result.ok {
            return Ok(());
        }
        let policy = exchange
            .replacement
            .as_ref()
            .map(|replacement| replacement.policy.as_str());
        if !should_maintain(mode, policy, visible_chars(exchange)) {
            return Ok(());
        }
        let visible = visible_text(exchange);
        // 1. 归档指针：沿用既有引用，双双缺失时先把模型可见原文落盘
        let result_ref = match existing_result_ref(exchange) {
            Some(reference) => reference.to_string(),
            None => self.write_tool_result_archive(&exchange.call.provider_call_id, visible)?,
        };
        let original_chars = original_chars_of(exchange);
        let replacement = match mode {
            ToolResultMaintenanceMode::Snip => snip_text(
                &exchange.call.tool_name,
                visible,
                original_chars,
                &result_ref,
            ),
            ToolResultMaintenanceMode::Prune => {
                prune_text(&exchange.call.tool_name, original_chars, &result_ref)
            }
        };
        let visible_len = visible.chars().count();
        let replacement_len = replacement.chars().count();
        // 2. 改写没有收益时保持原样，避免无意义的前缀缓存失效
        if replacement_len >= visible_len {
            return Ok(());
        }
        upsert_tool_output_replacement(
            &self.conv_db,
            NewToolOutputReplacement {
                provider_call_id: exchange.call.provider_call_id.clone(),
                session_id: self.session_id.clone(),
                replacement,
                original_chars,
                result_ref,
                policy: match mode {
                    ToolResultMaintenanceMode::Snip => POLICY_STALE_SNIP.to_string(),
                    ToolResultMaintenanceMode::Prune => POLICY_STALE_PRUNE.to_string(),
                },
            },
        )?;
        stats.rewritten += 1;
        stats.saved_chars += visible_len - replacement_len;
        Ok(())
    }
}

/// 判断记录是否需要在当前模式下改写。
///
/// 参数:
/// - `mode`: 裁剪或折叠
/// - `policy`: 现有 replacement 策略
/// - `visible_chars`: 当前模型可见字符数
///
/// 返回:
/// - 需要改写时为 true
fn should_maintain(
    mode: ToolResultMaintenanceMode,
    policy: Option<&str>,
    visible_chars: usize,
) -> bool {
    if policy == Some(POLICY_STALE_PRUNE) {
        return false;
    }
    match mode {
        ToolResultMaintenanceMode::Snip => {
            policy != Some(POLICY_STALE_SNIP) && visible_chars >= MIN_MAINTAIN_CHARS
        }
        ToolResultMaintenanceMode::Prune => {
            policy == Some(POLICY_STALE_SNIP) || visible_chars >= MIN_MAINTAIN_CHARS
        }
    }
}

/// 取模型当前可见的工具结果文本。
///
/// 参数:
/// - `exchange`: 工具交换记录
///
/// 返回:
/// - replacement 优先，其次 result_preview
fn visible_text(exchange: &ToolExchangeRecord) -> &str {
    if let Some(replacement) = &exchange.replacement {
        return &replacement.replacement;
    }
    exchange
        .result
        .as_ref()
        .map(|result| result.result_preview.as_str())
        .unwrap_or_default()
}

/// 取模型当前可见文本的字符数。
///
/// 参数:
/// - `exchange`: 工具交换记录
///
/// 返回:
/// - 可见字符数
fn visible_chars(exchange: &ToolExchangeRecord) -> usize {
    visible_text(exchange).chars().count()
}

/// 取已存在的完整输出归档引用。
///
/// 参数:
/// - `exchange`: 工具交换记录
///
/// 返回:
/// - replacement 或结果记录中的引用；都没有时为 None
fn existing_result_ref(exchange: &ToolExchangeRecord) -> Option<&str> {
    if let Some(replacement) = &exchange.replacement {
        if !replacement.result_ref.is_empty() {
            return Some(&replacement.result_ref);
        }
    }
    exchange.result.as_ref()?.result_ref.as_deref()
}

/// 取原始输出字符数。
///
/// 参数:
/// - `exchange`: 工具交换记录
///
/// 返回:
/// - 结果记录的 original_chars；缺省时退回当前可见字符数
fn original_chars_of(exchange: &ToolExchangeRecord) -> usize {
    let recorded = exchange
        .result
        .as_ref()
        .map(|result| result.original_chars)
        .unwrap_or_default();
    if recorded > 0 {
        recorded
    } else {
        visible_chars(exchange)
    }
}

/// 按工具类别决定裁剪几何。
///
/// 只读检索类工具的输出前重后轻（前几行就是答案），保长头短尾；
/// 有副作用的命令类工具两端都可能藏关键信息，保持均衡。
///
/// 参数:
/// - `tool_name`: 工具名称
///
/// 返回:
/// - 裁剪几何参数
fn snip_geometry(tool_name: &str) -> SnipGeometry {
    if BALANCED_SNIP_TOOLS.contains(&tool_name) {
        return SnipGeometry {
            head_lines: 20,
            tail_lines: 20,
            head_chars: 4_000,
            tail_chars: 4_000,
        };
    }
    SnipGeometry {
        head_lines: 40,
        tail_lines: 8,
        head_chars: 6_000,
        tail_chars: 1_200,
    }
}

/// 生成首尾裁剪后的替换文本。
///
/// 参数:
/// - `tool_name`: 工具名称
/// - `visible`: 当前可见文本
/// - `original_chars`: 原始输出字符数
/// - `result_ref`: 完整输出归档引用
///
/// 返回:
/// - 带标记行的裁剪文本
fn snip_text(tool_name: &str, visible: &str, original_chars: usize, result_ref: &str) -> String {
    let geometry = snip_geometry(tool_name);
    let lines = visible.lines().collect::<Vec<_>>();
    if lines.len() <= geometry.head_lines + geometry.tail_lines {
        // 行数不足以按行裁剪（超长单行）：按字符保首尾
        let head = take_chars(visible, geometry.head_chars);
        let tail = take_chars_from_end(visible, geometry.tail_chars);
        let omitted = visible
            .chars()
            .count()
            .saturating_sub(head.chars().count() + tail.chars().count());
        return format!(
            "[snipped stale tool result — {tool_name}, {original_chars} chars archived to {result_ref}; long line truncated]\n{head}\n[... {omitted} chars omitted ...]\n{tail}"
        );
    }
    let head = lines[..geometry.head_lines].join("\n");
    let tail = lines[lines.len() - geometry.tail_lines..].join("\n");
    let omitted = lines.len() - geometry.head_lines - geometry.tail_lines;
    format!(
        "[snipped stale tool result — {tool_name}, {original_chars} chars archived to {result_ref}; showing first {} and last {} lines]\n{head}\n[... {omitted} lines omitted ...]\n{tail}",
        geometry.head_lines, geometry.tail_lines
    )
}

/// 生成折叠后的一行占位文本。
///
/// 参数:
/// - `tool_name`: 工具名称
/// - `original_chars`: 原始输出字符数
/// - `result_ref`: 完整输出归档引用
///
/// 返回:
/// - 占位说明文本
fn prune_text(tool_name: &str, original_chars: usize, result_ref: &str) -> String {
    format!(
        "[elided stale tool result — {tool_name}, {original_chars} chars archived to {result_ref}; re-run the tool if the data is needed again]"
    )
}

/// 从头部取指定数量的字符。
///
/// 参数:
/// - `value`: 原始文本
/// - `count`: 保留字符数
///
/// 返回:
/// - 头部片段
fn take_chars(value: &str, count: usize) -> String {
    value.chars().take(count).collect()
}

/// 从尾部取指定数量的字符。
///
/// 参数:
/// - `value`: 原始文本
/// - `count`: 保留字符数
///
/// 返回:
/// - 尾部片段
fn take_chars_from_end(value: &str, count: usize) -> String {
    let total = value.chars().count();
    value.chars().skip(total.saturating_sub(count)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::tool_history::repository::{insert_tool_call, insert_tool_result};
    use crate::state::tool_history::{NewToolCallRecord, NewToolResultRecord};
    use crate::state::turns::ConversationDb;

    /// 构造带三个已完成轮次的测试状态仓库。
    ///
    /// 返回:
    /// - 临时目录与状态仓库；turn_1 是唯一超出保留尾部的陈旧轮次
    fn store_with_turns() -> (tempfile::TempDir, StateStore) {
        let temp = tempfile::tempdir().unwrap();
        let db = ConversationDb::open(temp.path()).unwrap();
        for seq in 1..=3 {
            let turn_id = format!("turn_{seq}");
            db.start_turn(&turn_id, "inspect").unwrap();
            db.complete_turn(&turn_id, "done", None).unwrap();
        }
        let store = StateStore {
            base_state_dir: temp.path().to_path_buf(),
            session_id: "default".to_string(),
            state_dir: temp.path().to_path_buf(),
            conv_db: std::sync::Arc::new(db),
        };
        (temp, store)
    }

    /// 写入指定轮次的一条工具调用与结果。
    ///
    /// 参数:
    /// - `store`: 状态仓库
    /// - `turn_id`: 轮次标识
    /// - `call_id`: provider 调用标识
    /// - `ok`: 结果是否成功
    /// - `preview`: 模型可见结果文本
    fn insert_exchange(store: &StateStore, turn_id: &str, call_id: &str, ok: bool, preview: &str) {
        insert_tool_call(
            &store.conv_db,
            NewToolCallRecord {
                session_id: "default".to_string(),
                turn_id: turn_id.to_string(),
                seq: 1,
                provider_call_id: call_id.to_string(),
                tool_name: "read_file".to_string(),
                arguments: "{}".to_string(),
            },
        )
        .unwrap();
        insert_tool_result(
            &store.conv_db,
            NewToolResultRecord {
                session_id: "default".to_string(),
                turn_id: turn_id.to_string(),
                provider_call_id: call_id.to_string(),
                ok,
                result_preview: preview.to_string(),
                result_ref: None,
                error: None,
                original_chars: preview.chars().count(),
            },
        )
        .unwrap();
    }

    /// 读取指定调用当前的 replacement 记录。
    ///
    /// 参数:
    /// - `store`: 状态仓库
    /// - `turn_id`: 轮次标识
    /// - `call_id`: provider 调用标识
    ///
    /// 返回:
    /// - replacement 记录；不存在时为 None
    fn replacement_of(
        store: &StateStore,
        turn_id: &str,
        call_id: &str,
    ) -> Option<super::super::model::ToolOutputReplacement> {
        load_tool_exchanges_for_turn(&store.conv_db, "default", turn_id)
            .unwrap()
            .into_iter()
            .find(|exchange| exchange.call.provider_call_id == call_id)
            .and_then(|exchange| exchange.replacement)
    }

    fn large_output() -> String {
        (0..200)
            .map(|index| format!("line {index} with some payload"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 验证已完成轮次的工具结果全部参与裁剪，且幂等。
    #[test]
    fn snip_rewrites_stale_results_and_preserves_recent_tail() {
        let (_temp, store) = store_with_turns();
        insert_exchange(&store, "turn_1", "call_stale", true, &large_output());
        insert_exchange(&store, "turn_3", "call_recent", true, &large_output());

        let stats = store
            .maintain_stale_tool_results(ToolResultMaintenanceMode::Snip)
            .unwrap();

        // 方案 B：已完成轮次不再保留尾部，两条结果都会被裁剪
        assert_eq!(stats.rewritten, 2);
        assert!(stats.saved_chars > 0);
        let replacement = replacement_of(&store, "turn_1", "call_stale").unwrap();
        assert_eq!(replacement.policy, POLICY_STALE_SNIP);
        assert!(replacement
            .replacement
            .contains("snipped stale tool result"));
        assert!(replacement.replacement.contains("line 0"));
        assert!(replacement.replacement.contains("line 199"));
        assert!(replacement_of(&store, "turn_3", "call_recent").is_some());

        let second = store
            .maintain_stale_tool_results(ToolResultMaintenanceMode::Snip)
            .unwrap();
        assert_eq!(second.rewritten, 0);
    }

    /// 验证运行中轮次的较早工具结果参与裁剪，末尾若干条保留。
    ///
    /// 这是旧策略完全失效的场景：单轮内大量工具调用撑爆上下文。
    #[test]
    fn snip_covers_running_turn_and_keeps_latest_calls() {
        let temp = tempfile::tempdir().unwrap();
        let db = ConversationDb::open(temp.path()).unwrap();
        db.start_turn("turn_running", "inspect").unwrap();
        let store = StateStore {
            base_state_dir: temp.path().to_path_buf(),
            session_id: "default".to_string(),
            state_dir: temp.path().to_path_buf(),
            conv_db: std::sync::Arc::new(db),
        };
        let total = crate::state::compaction::PRESERVED_RUNNING_TOOL_CALLS + 3;
        for index in 0..total {
            insert_exchange(
                &store,
                "turn_running",
                &format!("call_{index}"),
                true,
                &large_output(),
            );
        }

        let stats = store
            .maintain_stale_tool_results(ToolResultMaintenanceMode::Snip)
            .unwrap();

        assert_eq!(stats.rewritten, 3, "较早的三条被裁剪");
        assert!(replacement_of(&store, "turn_running", "call_0").is_some());
        let latest = format!("call_{}", total - 1);
        assert!(
            replacement_of(&store, "turn_running", &latest).is_none(),
            "末尾最新的结果必须保留"
        );
    }

    /// 验证折叠可以升级已裁剪结果，并保持幂等。
    #[test]
    fn prune_upgrades_snipped_result_to_placeholder() {
        let (_temp, store) = store_with_turns();
        insert_exchange(&store, "turn_1", "call_stale", true, &large_output());
        store
            .maintain_stale_tool_results(ToolResultMaintenanceMode::Snip)
            .unwrap();

        let stats = store
            .maintain_stale_tool_results(ToolResultMaintenanceMode::Prune)
            .unwrap();

        assert_eq!(stats.rewritten, 1);
        let replacement = replacement_of(&store, "turn_1", "call_stale").unwrap();
        assert_eq!(replacement.policy, POLICY_STALE_PRUNE);
        assert!(replacement.replacement.contains("elided stale tool result"));

        let second = store
            .maintain_stale_tool_results(ToolResultMaintenanceMode::Prune)
            .unwrap();
        assert_eq!(second.rewritten, 0);
    }

    /// 验证错误结果与小结果保持原文。
    #[test]
    fn keeps_error_and_small_results_verbatim() {
        let (_temp, store) = store_with_turns();
        insert_exchange(&store, "turn_1", "call_error", false, &large_output());
        insert_exchange(&store, "turn_1", "call_small", true, "short output");

        let stats = store
            .maintain_stale_tool_results(ToolResultMaintenanceMode::Prune)
            .unwrap();

        assert_eq!(stats.rewritten, 0);
        assert!(replacement_of(&store, "turn_1", "call_error").is_none());
        assert!(replacement_of(&store, "turn_1", "call_small").is_none());
    }

    /// 验证缺少归档引用时先落盘原文再改写。
    #[test]
    fn archives_visible_text_when_result_ref_missing() {
        let (temp, store) = store_with_turns();
        let output = large_output();
        insert_exchange(&store, "turn_1", "call_stale", true, &output);

        store
            .maintain_stale_tool_results(ToolResultMaintenanceMode::Snip)
            .unwrap();

        let replacement = replacement_of(&store, "turn_1", "call_stale").unwrap();
        let archived = std::fs::read_to_string(temp.path().join(&replacement.result_ref)).unwrap();
        assert_eq!(archived, output);
    }
}
