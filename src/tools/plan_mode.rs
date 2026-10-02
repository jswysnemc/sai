use super::{ToolRegistry, ToolSpec};
use serde_json::json;

/// 【计划模式】【工具注册】参数为工具注册表；仅登记会话操作，真实执行由 Agent 串行处理。
pub(crate) fn register(registry: &mut ToolRegistry) {
    registry.register(ToolSpec::new("enter_plan_mode", "Enter read-only Plan mode before designing a substantial implementation. Explore, clarify important choices with ask_question, then submit the complete plan with exit_plan_mode. Do not use this for trivial edits.", json!({"type":"object","properties":{},"additionalProperties":false}), |_| async { anyhow::bail!("enter_plan_mode requires an active Sai session") }));
    registry.register(ToolSpec::new("exit_plan_mode", "Submit the complete Markdown implementation plan for explicit user review. Include scope, affected files, steps, verification and risks. Approval exits Plan mode; feedback or cancellation keeps it read-only. Never infer approval from silence or another tool result.", json!({
        "type":"object", "properties":{
            "title":{"type":"string","minLength":1,"maxLength":80},
            "plan":{"type":"string","minLength":1,"maxLength":100000,"description":"Complete Markdown plan shown to the user, not a file path or summary."}
        },"required":["title","plan"],"additionalProperties":false
    }), |_| async { anyhow::bail!("exit_plan_mode requires an active Sai session") }));
}
