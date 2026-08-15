use super::AgentMode;
use crate::llm::{ChatContent, ChatContentPart, ChatMessage};
use anyhow::Result;
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::io::IsTerminal;
use std::path::Path;
use std::process::Command;

const STATE_OPEN: &str = "<context-state>";
const STATE_CLOSE: &str = "</context-state>";

/// 稳定系统提示中的状态覆盖规则。
pub(crate) const CONTEXT_STATE_CONTRACT: &str = "<context-state-contract>\n运行状态与权限模式可能在后续 user 角色的 <context-state> 标签中更新。以历史中最后一条对应状态为准；mode detail=full 后的 <mode-instructions> 是该模式约束，detail=switch 只切换到已载入模式。\n</context-state-contract>";

/// 当前运行状态快照。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RuntimeContextSnapshot {
    pub(crate) date: String,
    pub(crate) cwd: String,
    pub(crate) model: String,
    pub(crate) git_branch: String,
    pub(crate) shell: String,
    pub(crate) terminal: String,
}

impl RuntimeContextSnapshot {
    /// 采集当前运行状态。
    ///
    /// 参数:
    /// - `selected_model`: 当前 provider/model 标签
    ///
    /// 返回:
    /// - 限长后的状态快照
    pub(crate) fn capture(selected_model: Option<&str>) -> Self {
        let cwd = crate::runtime_cwd::current_dir().unwrap_or_else(|_| ".".into());
        Self {
            date: Local::now().format("%Y-%m-%d").to_string(),
            cwd: sanitize(&cwd.display().to_string(), 240),
            model: sanitize(selected_model.unwrap_or("unknown"), 120),
            git_branch: git_branch(&cwd).unwrap_or_else(|| "none".to_string()),
            shell: sanitize(
                &std::env::var("SHELL").unwrap_or_else(|_| "unknown".to_string()),
                80,
            ),
            terminal: terminal_label(),
        }
    }
}

/// 构造本轮需要追加的状态事件。
///
/// 参数:
/// - `snapshot`: 当前运行状态
/// - `mode`: 当前权限模式
/// - `checkpoint_context`: 当前压缩摘要上下文
/// - `history`: 压缩后仍可见的 provider 历史
/// - `include_mode_reminder`: 是否注入当前模式的约束说明
///
/// 返回:
/// - 首次全量或变化单项；没有变化时返回 None
pub(crate) fn context_state_update(
    snapshot: &RuntimeContextSnapshot,
    mode: AgentMode,
    checkpoint_context: Option<&str>,
    history: &[ChatMessage],
    include_mode_reminder: bool,
) -> Result<Option<String>> {
    let known = KnownContextState::from_projection(checkpoint_context, history);
    let mut parts = runtime_update_parts(snapshot, known.runtime.as_ref())?;
    if include_mode_reminder {
        if let Some(mode_update) = mode_update(mode, &known)? {
            parts.push(mode_update);
        }
    }
    if parts.is_empty() {
        Ok(None)
    } else {
        Ok(Some(parts.join("\n\n")))
    }
}

/// Provider 历史中记录的状态事件。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ContextStateRecord {
    Runtime {
        version: u8,
        date: String,
        cwd: String,
        model: String,
        git_branch: String,
        shell: String,
        terminal: String,
    },
    RuntimeChange {
        field: String,
        value: String,
    },
    Mode {
        name: String,
        detail: ModeDetail,
    },
}

/// 模式事件是否附带完整说明。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ModeDetail {
    Full,
    Switch,
}

/// 从当前可见历史恢复出的最新状态。
#[derive(Debug, Default)]
struct KnownContextState {
    runtime: Option<RuntimeContextSnapshot>,
    mode: Option<String>,
    detailed_modes: BTreeSet<String>,
}

impl KnownContextState {
    /// 从压缩摘要与历史消息恢复状态。
    ///
    /// 参数:
    /// - `checkpoint_context`: 当前压缩摘要上下文
    /// - `history`: 当前可见 provider 历史
    ///
    /// 返回:
    /// - 可确认仍在上下文中的状态
    fn from_projection(checkpoint_context: Option<&str>, history: &[ChatMessage]) -> Self {
        let mut state = Self::default();
        if let Some(context) = checkpoint_context {
            state.apply_text(context);
        }
        for message in history {
            if let Some(text) = message_text(message) {
                state.apply_text(&text);
            }
        }
        state
    }

    /// 应用一条消息中的全部状态记录。
    ///
    /// 参数:
    /// - `text`: provider 消息文本
    ///
    /// 返回:
    /// - 无
    fn apply_text(&mut self, text: &str) {
        for record in parse_records(text) {
            match record {
                ContextStateRecord::Runtime {
                    date,
                    cwd,
                    model,
                    git_branch,
                    shell,
                    terminal,
                    ..
                } => {
                    self.runtime = Some(RuntimeContextSnapshot {
                        date,
                        cwd,
                        model,
                        git_branch,
                        shell,
                        terminal,
                    });
                }
                ContextStateRecord::RuntimeChange { field, value } => {
                    if let Some(runtime) = self.runtime.as_mut() {
                        apply_runtime_change(runtime, &field, value);
                    }
                }
                ContextStateRecord::Mode { name, detail } => {
                    self.mode = Some(name.clone());
                    if detail == ModeDetail::Full && has_complete_mode_instructions(text, &name) {
                        self.detailed_modes.insert(name);
                    }
                }
            }
        }
    }
}

/// 构造运行状态的首次全量或变化单项。
///
/// 参数:
/// - `current`: 当前快照
/// - `previous`: 历史中的最近快照
///
/// 返回:
/// - 待追加的状态标签
fn runtime_update_parts(
    current: &RuntimeContextSnapshot,
    previous: Option<&RuntimeContextSnapshot>,
) -> Result<Vec<String>> {
    let Some(previous) = previous else {
        return Ok(vec![render_record(&ContextStateRecord::Runtime {
            version: 1,
            date: current.date.clone(),
            cwd: current.cwd.clone(),
            model: current.model.clone(),
            git_branch: current.git_branch.clone(),
            shell: current.shell.clone(),
            terminal: current.terminal.clone(),
        })?]);
    };
    let mut parts = Vec::new();
    for (field, old, new) in [
        ("cwd", previous.cwd.as_str(), current.cwd.as_str()),
        ("model", previous.model.as_str(), current.model.as_str()),
        (
            "git_branch",
            previous.git_branch.as_str(),
            current.git_branch.as_str(),
        ),
        ("shell", previous.shell.as_str(), current.shell.as_str()),
        (
            "terminal",
            previous.terminal.as_str(),
            current.terminal.as_str(),
        ),
    ] {
        if old != new {
            parts.push(render_record(&ContextStateRecord::RuntimeChange {
                field: field.to_string(),
                value: new.to_string(),
            })?);
        }
    }
    Ok(parts)
}

/// 构造模式首次说明或切换简报。
///
/// 参数:
/// - `mode`: 当前模式
/// - `known`: 历史中仍可见的状态
///
/// 返回:
/// - 模式没有变化且说明仍在时返回 None
fn mode_update(mode: AgentMode, known: &KnownContextState) -> Result<Option<String>> {
    let name = mode.key();
    let explanation_loaded = known.detailed_modes.contains(name);
    if known.mode.as_deref() == Some(name) && explanation_loaded {
        return Ok(None);
    }
    let detail = if explanation_loaded {
        ModeDetail::Switch
    } else {
        ModeDetail::Full
    };
    let record = render_record(&ContextStateRecord::Mode {
        name: name.to_string(),
        detail,
    })?;
    if detail == ModeDetail::Switch {
        Ok(Some(format!(
            "{record}\n已切换到 {} 模式；继续遵循此前载入的该模式说明。",
            mode.label()
        )))
    } else {
        Ok(Some(format!(
            "{record}\n<mode-instructions name=\"{name}\">\n{}\n</mode-instructions>",
            mode.reminder().trim()
        )))
    }
}

/// 将状态记录包装为固定标签。
///
/// 参数:
/// - `record`: 状态记录
///
/// 返回:
/// - 可注入 provider 用户消息的文本
fn render_record(record: &ContextStateRecord) -> Result<String> {
    Ok(format!(
        "{STATE_OPEN}\n{}\n{STATE_CLOSE}",
        serde_json::to_string(record)?
    ))
}

/// 解析消息中的全部状态记录。
///
/// 参数:
/// - `text`: provider 消息文本
///
/// 返回:
/// - 成功解析的记录；损坏片段会忽略
fn parse_records(text: &str) -> Vec<ContextStateRecord> {
    let mut records = Vec::new();
    let mut remaining = text;
    while let Some(start) = remaining.find(STATE_OPEN) {
        let after_open = &remaining[start + STATE_OPEN.len()..];
        let Some(end) = after_open.find(STATE_CLOSE) else {
            break;
        };
        if let Ok(record) = serde_json::from_str(after_open[..end].trim()) {
            records.push(record);
        }
        remaining = &after_open[end + STATE_CLOSE.len()..];
    }
    records
}

/// 判断完整模式说明是否仍保留在同一条消息中。
///
/// 参数:
/// - `text`: provider 消息文本
/// - `name`: 模式名称
///
/// 返回:
/// - 起止标签均存在时返回 true
fn has_complete_mode_instructions(text: &str, name: &str) -> bool {
    let open = format!("<mode-instructions name=\"{name}\">");
    let Some(start) = text.find(&open) else {
        return false;
    };
    let body = &text[start + open.len()..];
    let Some(end) = body.find("</mode-instructions>") else {
        return false;
    };
    // 压缩摘要可能只保留某个模式的起始标签；不能借用后续模式的结束标签
    body.find("<context-state>")
        .is_none_or(|next_state| end < next_state)
}

/// 把单项变化应用到已知快照。
///
/// 参数:
/// - `runtime`: 待更新快照
/// - `field`: 字段名
/// - `value`: 新值
///
/// 返回:
/// - 无
fn apply_runtime_change(runtime: &mut RuntimeContextSnapshot, field: &str, value: String) {
    match field {
        "cwd" => runtime.cwd = value,
        "model" => runtime.model = value,
        "git_branch" => runtime.git_branch = value,
        "shell" => runtime.shell = value,
        "terminal" => runtime.terminal = value,
        _ => {}
    }
}

/// 提取聊天消息文本。
///
/// 参数:
/// - `message`: 聊天消息
///
/// 返回:
/// - 不含图片的文本
fn message_text(message: &ChatMessage) -> Option<String> {
    match message.content.as_ref() {
        Some(ChatContent::Text(text)) => Some(text.clone()),
        Some(ChatContent::Parts(parts)) => Some(
            parts
                .iter()
                .filter_map(|part| match part {
                    ChatContentPart::Text { text } => Some(text.as_str()),
                    ChatContentPart::ImageUrl { .. } => None,
                })
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        None => None,
    }
}

/// 读取当前工作区的 Git 分支。
///
/// 参数:
/// - `cwd`: 当前工作目录
///
/// 返回:
/// - 分支名称；非 Git 目录或 detached HEAD 返回 None
fn git_branch(cwd: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["symbolic-ref", "--quiet", "--short", "HEAD"])
        .env("GIT_OPTIONAL_LOCKS", "0")
        .current_dir(cwd)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let branch = String::from_utf8_lossy(&output.stdout);
    let branch = sanitize(branch.trim(), 120);
    (!branch.is_empty()).then_some(branch)
}

/// 将终端环境压缩成单个标签。
///
/// 返回:
/// - 交互类型和终端标识
fn terminal_label() -> String {
    let interactive = std::io::stdin().is_terminal()
        || std::io::stdout().is_terminal()
        || std::io::stderr().is_terminal();
    let kind = if interactive { "interactive" } else { "pipe" };
    let identity = ["TERM_PROGRAM", "TERM", "COLORTERM"]
        .into_iter()
        .filter_map(|key| {
            std::env::var(key)
                .ok()
                .filter(|value| !value.trim().is_empty())
                .map(|value| format!("{key}={value}"))
        })
        .collect::<Vec<_>>()
        .join(",");
    sanitize(
        &if identity.is_empty() {
            kind.to_string()
        } else {
            format!("{kind}:{identity}")
        },
        160,
    )
}

/// 清理控制字符并限制动态字段长度。
///
/// 参数:
/// - `value`: 原始字段
/// - `limit`: 最大字符数
///
/// 返回:
/// - 可安全放入上下文的单行文本
fn sanitize(value: &str, limit: usize) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .take(limit)
        .collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造固定运行状态。
    ///
    /// 参数:
    /// - `model`: 模型标签
    /// - `branch`: Git 分支
    ///
    /// 返回:
    /// - 测试快照
    fn snapshot(model: &str, branch: &str) -> RuntimeContextSnapshot {
        RuntimeContextSnapshot {
            date: "2026-08-08".to_string(),
            cwd: "/workspace/project".to_string(),
            model: model.to_string(),
            git_branch: branch.to_string(),
            shell: "/bin/zsh".to_string(),
            terminal: "interactive:xterm".to_string(),
        }
    }

    /// 验证首次注入包含全量运行状态和完整模式说明。
    #[test]
    fn initial_update_contains_full_runtime_and_mode() {
        let update = context_state_update(&snapshot("p/m", "main"), AgentMode::Yolo, None, &[], true)
            .unwrap()
            .unwrap();

        assert!(update.contains("\"kind\":\"runtime\""));
        assert!(update.contains("\"date\":\"2026-08-08\""));
        assert!(update.contains("\"kind\":\"mode\""));
        assert!(update.contains("\"detail\":\"full\""));
        assert!(update.contains("<mode-instructions name=\"yolo\">"));
    }

    /// 验证状态完全相同时不重复注入。
    #[test]
    fn unchanged_state_does_not_append_context() {
        let current = snapshot("p/m", "main");
        let first = context_state_update(&current, AgentMode::Yolo, None, &[], true)
            .unwrap()
            .unwrap();
        let history = vec![ChatMessage::plain("user", first)];

        assert!(
            context_state_update(&current, AgentMode::Yolo, None, &history, true)
                .unwrap()
                .is_none()
        );
    }

    /// 验证 Git 分支变化只追加该字段，日期不参与后续更新。
    #[test]
    fn branch_change_appends_only_changed_field() {
        let first = context_state_update(&snapshot("p/m", "main"), AgentMode::Yolo, None, &[], true)
            .unwrap()
            .unwrap();
        let history = vec![ChatMessage::plain("user", first)];

        let update = context_state_update(
            &snapshot("p/m", "feature/cache"),
            AgentMode::Yolo,
            None,
            &history,
            true,
        )
        .unwrap()
        .unwrap();

        assert!(update.contains("\"kind\":\"runtime_change\""));
        assert!(update.contains("\"field\":\"git_branch\""));
        assert!(!update.contains("\"field\":\"model\""));
        assert!(!update.contains("\"date\""));
    }

    /// 验证首次切入新模式载入完整说明，切回已载入模式只发送简报。
    #[test]
    fn mode_explanation_is_loaded_once_while_visible() {
        let runtime = snapshot("p/m", "main");
        let yolo = context_state_update(&runtime, AgentMode::Yolo, None, &[], true)
            .unwrap()
            .unwrap();
        let audited = context_state_update(
            &runtime,
            AgentMode::Audited,
            None,
            &[ChatMessage::plain("user", &yolo)],
            true,
        )
        .unwrap()
        .unwrap();
        let history = vec![
            ChatMessage::plain("user", yolo),
            ChatMessage::plain("user", audited),
        ];

        let switched = context_state_update(&runtime, AgentMode::Yolo, None, &history, true)
            .unwrap()
            .unwrap();

        assert!(switched.contains("\"detail\":\"switch\""));
        assert!(!switched.contains("<mode-instructions name=\"yolo\">"));
    }

    /// 验证压缩移除模式说明后会重新发送完整说明。
    #[test]
    fn missing_mode_explanation_after_compaction_is_reloaded() {
        let update = context_state_update(
            &snapshot("p/m", "main"),
            AgentMode::Audited,
            Some("<conversation-handoff>summary only</conversation-handoff>"),
            &[],
            true,
        )
        .unwrap()
        .unwrap();

        assert!(update.contains("\"detail\":\"full\""));
        assert!(update.contains("<mode-instructions name=\"audited\">"));
    }

    /// 验证动态字段清理控制字符并执行限长。
    #[test]
    fn sanitizes_multiline_dynamic_values() {
        assert_eq!(sanitize("a\nb\tc", 20), "abc");
        assert_eq!(sanitize("abcdef", 3), "abc");
    }
}
