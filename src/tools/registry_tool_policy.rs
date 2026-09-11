use super::{ToolProgress, ToolRegistry};
use sai_plugin_runtime::ToolPolicyInput;
use serde_json::Value;
use std::{collections::BTreeMap, time::Duration};

/// 【工具策略】【循环状态】状态绑定会话、操作、目录及实例，离开工具循环后丢弃
#[derive(Default)]
pub(crate) struct PluginToolPolicyStates {
    binding: Option<(String, String, String, String)>,
    entries: BTreeMap<String, (u64, Value)>,
}

impl ToolRegistry {
    /// 【工具策略】【有界分发】实际工具完成后执行只读策略，错误不改写原工具结果
    /// @param states 当前循环状态；name 为工具名称；arguments 为原参数；ok 为实际结果
    /// @returns 最多八条当前请求提醒
    pub(crate) async fn after_plugin_tool(
        &self,
        states: &mut PluginToolPolicyStates,
        name: &str,
        arguments: &str,
        ok: bool,
    ) -> Vec<String> {
        let context = self.plugin_context(ToolProgress::default(), false);
        let binding = (
            context.session_id.clone(),
            context.storage_session_id.clone(),
            context.operation_id.clone(),
            context.workdir.clone(),
        );
        if states.binding.as_ref() != Some(&binding) {
            states.entries.clear();
            states.binding = Some(binding);
        }
        let policies = self.plugins.tool_policies();
        states.entries.retain(|id, (instance, _)| {
            policies
                .iter()
                .any(|(current, owner, _)| current == id && owner == instance)
        });
        if policies.is_empty() {
            return Vec::new();
        }
        let arguments = serde_json::from_str(arguments).unwrap_or(Value::Null);
        let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
        let mut reminders = Vec::new();
        for (id, instance, names) in policies.into_iter().take(8) {
            let input = ToolPolicyInput {
                name: name.into(),
                local_name: names
                    .iter()
                    .find(|(_, public)| public == name)
                    .map(|(local, _)| local.clone()),
                arguments: arguments.clone(),
                ok,
                tools: names
                    .into_iter()
                    .filter(|(_, public)| self.contains(public))
                    .map(|(local, _)| local)
                    .collect(),
            };
            let previous = states
                .entries
                .get(&id)
                .map(|(_, state)| state.clone())
                .unwrap_or(Value::Null);
            match tokio::time::timeout_at(
                deadline,
                self.plugins
                    .after_tool(&id, input, previous, context.clone()),
            )
            .await
            {
                Ok(Ok(output)) => {
                    states.entries.insert(id, (instance, output.state));
                    if let Some(reminder) = output.reminder {
                        reminders.push(reminder);
                    }
                }
                Ok(Err(error)) => eprintln!("【工具策略】【回调失败】{id}: {error:#}"),
                Err(error) => {
                    eprintln!("【工具策略】【回调超时】{id}: {error}");
                    break;
                }
            }
        }
        reminders
    }
}
