use crate::render::tool_event_line::{tool_event_label, tool_event_text};
use crate::render::ToolCallDisplayMode;
use crate::tools::subagent_state::SubagentSnapshot;
use serde_json::Value;
use std::hash::{Hash, Hasher};

/// 子智能体内部的可见进度单元。
#[derive(Clone, Debug, Eq, PartialEq)]
enum SubagentPart {
    Reasoning(String),
    ToolCall {
        name: String,
        arguments: String,
    },
    ToolResult {
        name: String,
        ok: bool,
        output: String,
    },
    Progress(String),
}

/// 可持续更新的子智能体 TUI 单元。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SubagentCell {
    arguments: String,
    subagent_id: Option<String>,
    parts: Vec<SubagentPart>,
    outcome: Option<(bool, String)>,
}

impl SubagentCell {
    /// 创建正在执行的子智能体单元。
    ///
    /// 参数:
    /// - `arguments`: subagent 工具参数
    ///
    /// 返回:
    /// - 空时间线单元
    pub(crate) fn new(arguments: String) -> Self {
        Self {
            arguments,
            subagent_id: None,
            parts: Vec::new(),
            outcome: None,
        }
    }

    /// 记录一条子智能体进度。
    ///
    /// 参数:
    /// - `message`: 带内部类型前缀的进度文本
    ///
    /// 返回:
    /// - 无
    pub(crate) fn push_progress(&mut self, message: String) {
        let part = if let Some(text) = message.strip_prefix("__subagent_reasoning__") {
            SubagentPart::Reasoning(text.to_string())
        } else if let Some(payload) = message.strip_prefix("__subtool_call__") {
            parse_subtool_call(payload).unwrap_or_else(|| SubagentPart::Progress(message.clone()))
        } else if let Some(payload) = message.strip_prefix("__subtool_result__") {
            parse_subtool_result(payload).unwrap_or_else(|| SubagentPart::Progress(message.clone()))
        } else if message == "__external_output__" {
            return;
        } else {
            SubagentPart::Progress(message)
        };
        self.parts.push(part);
    }

    /// 完成子智能体单元。
    ///
    /// 参数:
    /// - `ok`: 执行是否成功
    /// - `output`: 最终结果
    ///
    /// 返回:
    /// - 无
    pub(crate) fn finish(&mut self, ok: bool, output: String) {
        // 【TUI】【子智能体绑定】1. 后台启动结果只绑定 ID，终态由持久化时间线驱动
        if ok {
            if let Some((id, status)) = subagent_identity(&output) {
                self.subagent_id = Some(id);
                if status == "running" {
                    return;
                }
            }
        }
        self.outcome = Some((ok, output));
    }

    /// 判断子智能体是否仍在执行。
    ///
    /// 返回:
    /// - 尚未完成时返回 true
    pub(crate) fn is_active(&self) -> bool {
        self.outcome.is_none() && self.subagent_id.is_none()
    }

    /// 判断后台子智能体是否仍会产生新时间线。
    ///
    /// 返回:
    /// - 子智能体仍在执行时返回 true
    pub(crate) fn has_live_updates(&self) -> bool {
        self.subagent_id
            .as_deref()
            .and_then(|id| crate::tools::subagent_state::subagent_snapshot(id).ok())
            .is_some_and(|snapshot| snapshot.status == "running")
    }

    /// 返回用于判断 TUI 是否需要重绘的状态签名。
    ///
    /// 返回:
    /// - `(ID, 状态, 更新时间, 时间线哈希)`
    pub(crate) fn state_signature(&self) -> Option<(String, String, u64, u64)> {
        // 【TUI】【子智能体重绘】1. 同时纳入快照和时间线，避免同一秒内的流式更新被忽略
        let id = self.subagent_id.as_deref()?;
        let snapshot = crate::tools::subagent_state::subagent_snapshot(id).ok()?;
        let timeline = crate::tools::subagent_state::subagent_timeline(id).ok()?;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        serde_json::to_string(&timeline).ok()?.hash(&mut hasher);
        Some((
            id.to_string(),
            snapshot.status,
            snapshot.updated_at,
            hasher.finish(),
        ))
    }
}

/// 渲染子智能体思考、工具和结果。
///
/// 参数:
/// - `cell`: 子智能体时间线
/// - `mode`: 工具展示模式
///
/// 返回:
/// - 复用主智能体工具视觉语言的 ANSI 文本
pub(super) fn render(cell: &SubagentCell, mode: ToolCallDisplayMode) -> String {
    if mode == ToolCallDisplayMode::Hidden {
        return String::new();
    }
    // 【TUI】【子智能体渲染】1. 优先使用后台持久化快照，工具内进度仅作前台兼容
    let snapshot = cell
        .subagent_id
        .as_deref()
        .and_then(|id| crate::tools::subagent_state::subagent_snapshot(id).ok());
    // 【TUI】【子智能体渲染】2. 使用主智能体工具标题和状态样式
    let label = snapshot
        .as_ref()
        .map(|snapshot| format!("Subagent {}", snapshot.description))
        .unwrap_or_else(|| tool_event_label("subagent", Some(&cell.arguments)));
    let status = snapshot
        .as_ref()
        .map(|snapshot| match snapshot.status.as_str() {
            "completed" => "ok",
            "failed" | "cancelled" => "err",
            _ => "run",
        })
        .unwrap_or_else(|| match cell.outcome.as_ref() {
            Some((true, _)) => "ok",
            Some((false, _)) => "err",
            None => "run",
        });
    let mut output = tool_event_text(&label, status);
    // 子 agent 响应不进入 transcript，避免与主 agent 流式渲染竞态；详情见底栏状态
    if let Some(summary) = compact_subagent_summary(cell, snapshot.as_ref()) {
        output.push_str(&format!("\n\x1b[2m  └─ {summary}\x1b[0m"));
    }
    let _ = mode;
    output
}

/// 生成子智能体的一行简要状态。
///
/// 参数:
/// - `cell`: 子智能体单元
/// - `snapshot`: 可选后台快照
///
/// 返回:
/// - 简要状态文本
fn compact_subagent_summary(
    cell: &SubagentCell,
    snapshot: Option<&SubagentSnapshot>,
) -> Option<String> {
    if let Some(snapshot) = snapshot {
        let status = match snapshot.status.as_str() {
            "running" => "运行中",
            "completed" => "已完成",
            "failed" => "失败",
            "cancelled" => "已取消",
            other => other,
        };
        return Some(match snapshot.last_tool.as_deref() {
            Some(tool) if !tool.is_empty() => format!("{status} · 最近工具 {tool}"),
            _ => format!("{status} · {}", snapshot.description),
        });
    }
    if cell.outcome.is_some() {
        return Some("已结束".to_string());
    }
    Some("运行中".to_string())
}

/// 从 subagent 工具结果中读取子智能体 ID 和状态。
///
/// 参数:
/// - `output`: subagent 工具 JSON 输出
///
/// 返回:
/// - 可识别时返回 `(ID, 状态)`
fn subagent_identity(output: &str) -> Option<(String, String)> {
    let value = serde_json::from_str::<Value>(output).ok()?;
    let subagent = value.get("subagent")?;
    Some((
        subagent.get("id")?.as_str()?.to_string(),
        subagent.get("status")?.as_str()?.to_string(),
    ))
}

/// 解析子工具调用进度。
///
/// 参数:
/// - `payload`: `{name, args}` JSON 文本
///
/// 返回:
/// - 可识别时返回工具调用单元
fn parse_subtool_call(payload: &str) -> Option<SubagentPart> {
    let value = serde_json::from_str::<Value>(payload).ok()?;
    Some(SubagentPart::ToolCall {
        name: value.get("name")?.as_str()?.to_string(),
        arguments: value
            .get("args")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    })
}

/// 解析子工具结果进度。
///
/// 参数:
/// - `payload`: `{name, ok, output}` JSON 文本
///
/// 返回:
/// - 可识别时返回工具结果单元
fn parse_subtool_result(payload: &str) -> Option<SubagentPart> {
    let value = serde_json::from_str::<Value>(payload).ok()?;
    Some(SubagentPart::ToolResult {
        name: value.get("name")?.as_str()?.to_string(),
        ok: value.get("ok").and_then(Value::as_bool).unwrap_or(true),
        output: value
            .get("output")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_compact_status_without_response_body() {
        let mut cell = SubagentCell::new(r#"{"description":"检查项目"}"#.to_string());
        cell.push_progress("__subagent_reasoning__先检查文件".to_string());
        cell.push_progress(
            r#"__subtool_call__{"name":"read_file","args":"{\"path\":\"README.md\"}"}"#.to_string(),
        );
        cell.push_progress(
            r#"__subtool_result__{"name":"read_file","ok":true,"output":"done"}"#.to_string(),
        );

        let rendered = render(&cell, ToolCallDisplayMode::Full);

        assert!(rendered.contains("subagent") || rendered.contains("Subagent"));
        assert!(rendered.contains("运行中") || rendered.contains("已结束"));
        assert!(!rendered.contains("先检查文件"));
        assert!(!rendered.contains("done"));
    }
}
