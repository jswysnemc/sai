use super::model::ToolView;
use crate::render::status_style::ToolHealth;
use crate::render::tool_event_line::{tool_event_label_tense, tool_status_line, ToolVerbTense};
use crate::render::ToolCallDisplayMode;
use serde::Deserialize;

/// `request_capability` 成功结果中的一项工具。
#[derive(Deserialize)]
struct ExposedTool {
    name: String,
}

/// `request_capability` 成功结果中的一项 skill。
#[derive(Deserialize)]
struct ExposedSkill {
    name: String,
}

/// Jev 能力申请的成功结果。Schema 与 skill 全文不进入终端。
#[derive(Deserialize)]
struct CapabilityResult {
    router: String,
    #[serde(default)]
    tools: Vec<ExposedTool>,
    #[serde(default)]
    skills: Vec<ExposedSkill>,
}

/// 渲染已完成的能力申请。
///
/// 运行中的调用仍走通用活动行。结果无法识别时返回空，交给通用载荷渲染。
///
/// 参数:
/// - `view`: 工具生命周期
/// - `mode`: 工具展示模式
///
/// 返回:
/// - 可识别时返回暴露名单
pub(super) fn render(view: &ToolView, mode: ToolCallDisplayMode) -> Option<String> {
    if mode == ToolCallDisplayMode::Hidden {
        return Some(String::new());
    }
    let outcome = view.outcome.as_ref()?;
    if !outcome.ok {
        return None;
    }
    let label = tool_event_label_tense(
        "request_capability",
        Some(&view.arguments),
        ToolVerbTense::Perfect,
    );
    render_capability_output(&label, &outcome.output, mode)
}

/// 从能力申请的原始结果渲染暴露名单。
///
/// 流式路径只缓存了事件标签，拿不到参数原文，因此标签由调用方给出。
///
/// 参数:
/// - `label`: 已生成的工具事件标签
/// - `result_json`: 工具结果 JSON
/// - `mode`: 工具展示模式
///
/// 返回:
/// - 结果可解析时返回名单文本
pub(crate) fn render_capability_output(
    label: &str,
    result_json: &str,
    mode: ToolCallDisplayMode,
) -> Option<String> {
    if mode == ToolCallDisplayMode::Hidden {
        return Some(String::new());
    }
    let result = serde_json::from_str::<CapabilityResult>(result_json).ok()?;
    if result.router != "jev" {
        return None;
    }
    let count = exposure_count(result.tools.len(), result.skills.len());
    let badge = format!("\x1b[2m{count}\x1b[0m");
    let mut output = tool_status_line(label, &badge, ToolHealth::Ok);
    if result.tools.is_empty() && result.skills.is_empty() {
        return Some(output);
    }
    for tool in &result.tools {
        let name = tool.name.trim();
        if name.is_empty() {
            continue;
        }
        output.push_str(&format!("\n\x1b[2m    tool {name}\x1b[0m"));
    }
    for skill in &result.skills {
        let name = skill.name.trim();
        if name.is_empty() {
            continue;
        }
        output.push_str(&format!("\n\x1b[2m    skill {name}\x1b[0m"));
    }
    Some(output)
}

/// 组装状态行上的暴露计数。
///
/// 参数:
/// - `tools`: 工具数量
/// - `skills`: skill 数量
///
/// 返回:
/// - 计数文本；都为零时表示未匹配
fn exposure_count(tools: usize, skills: usize) -> String {
    let mut parts = Vec::new();
    if tools > 0 {
        parts.push(if tools == 1 {
            "1 tool".to_string()
        } else {
            format!("{tools} tools")
        });
    }
    if skills > 0 {
        parts.push(if skills == 1 {
            "1 skill".to_string()
        } else {
            format!("{skills} skills")
        });
    }
    if parts.is_empty() {
        "no match".to_string()
    } else {
        parts.join(" · ")
    }
}
