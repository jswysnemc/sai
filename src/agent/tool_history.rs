use super::Agent;
use crate::llm::ToolCall;
use anyhow::Result;

impl Agent {
    /// 记录工具调用开始事件。
    ///
    /// 参数:
    /// - `turn_id`: 当前轮次标识
    /// - `seq`: 当前轮内工具调用顺序
    /// - `assistant_round`: 产生工具调用的模型子轮编号
    /// - `assistant_reasoning`: 该子轮的思考内容
    /// - `call`: provider 工具调用
    ///
    /// 返回:
    /// - 写入是否成功
    pub(super) fn record_tool_call_started(
        &self,
        turn_id: &str,
        seq: usize,
        assistant_round: usize,
        assistant_reasoning: Option<&str>,
        call: &ToolCall,
    ) -> Result<()> {
        self.state.record_tool_call_started_with_context(
            turn_id,
            seq,
            assistant_round,
            assistant_reasoning,
            &call.id,
            &call.function.name,
            &call.function.arguments,
        )
    }

    /// 记录工具网关解包后的真实展示信息。
    ///
    /// 参数:
    /// - `provider_call_id`: provider 工具调用标识
    /// - `call`: 解包后的真实工具调用
    ///
    /// 返回:
    /// - 写入是否成功
    pub(super) fn record_tool_call_display(
        &self,
        provider_call_id: &str,
        call: &ToolCall,
    ) -> Result<()> {
        self.state.record_tool_call_display(
            provider_call_id,
            &call.function.name,
            &call.function.arguments,
        )
    }

    /// 记录工具调用结果事件。
    ///
    /// 参数:
    /// - `turn_id`: 当前轮次标识
    /// - `call`: provider 工具调用
    /// - `ok`: 工具是否成功
    /// - `raw_output`: 原始工具输出
    /// - `context_output`: 模型可见工具输出
    ///
    /// 返回:
    /// - 写入是否成功
    pub(super) fn record_tool_result_completed(
        &self,
        turn_id: &str,
        call: &ToolCall,
        ok: bool,
        raw_output: &str,
        context_output: &str,
    ) -> Result<()> {
        let error = (!ok).then_some(raw_output);
        let result_ref = self.state.save_clipped_tool_output_replacement(
            &call.id,
            raw_output,
            context_output,
        )?;
        self.state.record_tool_result_completed(
            turn_id,
            &call.id,
            ok,
            context_output,
            result_ref.as_deref(),
            error,
            raw_output.chars().count(),
        )
    }

    /// 记录无需区分原始输出与上下文输出的工具结果。
    ///
    /// 参数:
    /// - `turn_id`: 当前轮次标识
    /// - `call`: 供应商原始工具调用
    /// - `ok`: 工具是否成功
    /// - `output`: 同时用于持久化和模型上下文的输出
    ///
    /// 返回:
    /// - 写入是否成功
    pub(super) fn record_simple_tool_result(
        &self,
        turn_id: &str,
        call: &ToolCall,
        ok: bool,
        output: &str,
    ) -> Result<()> {
        self.record_tool_result_completed(turn_id, call, ok, output, output)
    }
}

/// 提取需要跨轮保留的工具最终报告。
///
/// 参数:
/// - `tool_name`: 工具名称
/// - `output`: 工具 JSON 输出
///
/// 返回:
/// - 可持久化最终报告
pub(super) fn extract_persistable_tool_report(tool_name: &str, output: &str) -> Option<String> {
    let field = match tool_name {
        "linux_game_compatibility" => "final_report",
        "linux_input_method_diagnose" | "deep_diagnose" => "final_answer",
        _ => return None,
    };
    serde_json::from_str::<serde_json::Value>(output)
        .ok()
        .and_then(|value| {
            value
                .get(field)
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .map(str::to_string)
        })
        .filter(|report| !report.is_empty())
}
