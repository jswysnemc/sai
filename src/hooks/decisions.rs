//! 按事件解释输出，前置工具允许不能代替用户审批，后置反馈不能撤销副作用。

use super::HookEvent;
use serde_json::Value;

#[derive(Default, Debug, Clone)]
pub(crate) struct HookOutcome {
    pub(crate) deny: Option<String>,
    pub(crate) context: Vec<String>,
    pub(crate) updated_input: Option<Value>,
    pub(crate) permission_allow: bool,
    pub(crate) stop: bool,
    pub(crate) messages: Vec<String>,
}
impl HookOutcome {
    /// 【Hooks】【结果合并】参数为另一个结果，拒绝和停止优先，返回无值
    pub(super) fn merge(&mut self, other: Self) {
        self.deny = other.deny.or(self.deny.take());
        self.context.extend(other.context);
        self.messages.extend(other.messages);
        self.updated_input = other.updated_input.or(self.updated_input.take());
        self.permission_allow |= other.permission_allow;
        self.stop |= other.stop;
    }
}

/// 【Hooks】【事件决策】参数为事件、结构化输出和退出二反馈，返回该事件支持的控制效果
pub(super) fn interpret(event: HookEvent, value: Value, exit_two: Option<String>) -> HookOutcome {
    let mut result = HookOutcome::default();
    if let Some(text) = value["systemMessage"]
        .as_str()
        .filter(|text| !text.trim().is_empty())
    {
        result.messages.push(text.into());
    }
    let specific = &value["hookSpecificOutput"];
    if specific["hookEventName"]
        .as_str()
        .is_some_and(|name| name != event.as_str())
    {
        return result;
    }
    let reason = specific["permissionDecisionReason"]
        .as_str()
        .or(value["reason"].as_str())
        .filter(|text| !text.trim().is_empty())
        .map(str::to_string);
    let block = value["decision"] == "block";
    let stopped = value["continue"] == false;
    if matches!(
        event,
        HookEvent::PreToolUse
            | HookEvent::PostToolUse
            | HookEvent::SessionStart
            | HookEvent::UserPromptSubmit
            | HookEvent::SubagentStart
    ) {
        if let Some(context) = specific["additionalContext"]
            .as_str()
            .filter(|text| !text.trim().is_empty())
        {
            result.context.push(context.into());
        }
    }
    match event {
        HookEvent::PreToolUse => {
            if specific["permissionDecision"] == "deny" || block {
                result.deny = reason;
            }
            result.deny = exit_two.or(result.deny);
            if specific["permissionDecision"] == "allow" && specific["updatedInput"].is_object() {
                result.updated_input = Some(specific["updatedInput"].clone());
            }
        }
        HookEvent::PermissionRequest => {
            let decision = &specific["decision"];
            if decision.get("updatedInput").is_some()
                || decision.get("updatedPermissions").is_some()
                || decision.get("interrupt").is_some()
            {
                return result;
            }
            if decision["behavior"] == "allow" {
                result.permission_allow = true;
            }
            if decision["behavior"] == "deny" {
                result.deny = Some(
                    decision["message"]
                        .as_str()
                        .unwrap_or("Denied by permission hook")
                        .into(),
                );
            }
            result.deny = exit_two.or(result.deny);
        }
        HookEvent::PostToolUse => {
            if block || stopped {
                result.deny = reason.or_else(|| Some("Post-tool hook feedback".into()));
            }
            result.deny = exit_two.or(result.deny);
        }
        HookEvent::Stop | HookEvent::SubagentStop => {
            result.stop = stopped;
            if block {
                result.deny = reason;
            }
            result.deny = exit_two.or(result.deny);
        }
        HookEvent::UserPromptSubmit => {
            result.stop = stopped;
            if block {
                result.deny = reason;
            }
            result.deny = exit_two.or(result.deny);
        }
        HookEvent::SessionStart | HookEvent::PreCompact | HookEvent::PostCompact => {
            result.stop = stopped;
        }
        _ => {}
    }
    result
}
