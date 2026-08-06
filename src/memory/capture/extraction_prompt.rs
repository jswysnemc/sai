/// 记忆抽取的提示词与输出解析。
///
/// 旧实现用字符串裁剪冒充提炼：去填充词、找结论句、截断长度，产出的是
/// 「时间：任务 → 结果」的对话流水账。这里改为让模型判断哪些信息值得长期
/// 复用，并输出结构化候选。
use super::super::model::{MemoryCandidate, MemoryKind, MemoryScope};
use anyhow::Result;
use serde_json::Value;

/// 抽取模型的系统提示。
pub const EXTRACTION_SYSTEM_PROMPT: &str = r#"你从一轮对话中抽取值得长期记住的信息。

只抽取满足以下全部条件的内容：
- 在未来的、无关的对话中仍然有用
- 无法从代码仓库、文件内容或工具输出中直接读到
- 是关于用户本人、其工作方式或已确定的决策，而非这轮对话的过程

绝对不要抽取：
- 对本轮对话过程的复述（"用户询问了……"、"助手解释了……"）
- 工具执行的中间结果、文件内容、命令输出
- 临时性的、只在本次任务内有效的信息
- 可以随时重新查到的客观知识

输出 JSON 对象，字段如下：
{
  "memories": [
    {
      "kind": "preference | fact | decision | episode",
      "content": "一句陈述，主语明确，不含指代词",
      "salience": 0.0 到 1.0,
      "scope": "global | project",
      "tags": ["检索用的同义词，正文中没出现的才写"]
    }
  ]
}

kind 的含义：
- preference：用户稳定的偏好与习惯
- fact：关于用户或其环境的客观事实
- decision：已经做出的技术决策及其理由
- episode：发生过的、结果值得记住的事

salience 表示这条信息在半年后还有多大概率被用到。日常操作填 0.3 以下，
影响后续工作方式的填 0.7 以上。

scope 填 project 表示只在当前项目有效，填 global 表示跨项目通用。

没有任何值得记住的内容时输出 {"memories": []}。宁可少抽，不要凑数。"#;

/// 构造抽取请求的用户消息。
///
/// 参数:
/// - `user_message`: 用户原文
/// - `assistant_message`: 助手回复原文
/// - `workspace`: 当前工作区路径；无工作区时为 None
///
/// 返回:
/// - 提交给抽取模型的用户消息
pub fn build_extraction_input(
    user_message: &str,
    assistant_message: &str,
    workspace: Option<&str>,
) -> String {
    let workspace_line = match workspace {
        Some(path) => format!("当前工作区：{path}\n\n"),
        None => String::new(),
    };
    format!(
        "{workspace_line}用户：\n{}\n\n助手：\n{}",
        user_message.trim(),
        assistant_message.trim()
    )
}

/// 解析抽取模型的输出。
///
/// 模型偶尔会在 JSON 前后附带解释文字，这里先定位 JSON 对象再解析。
/// 单条候选解析失败时跳过该条而不是整体失败，避免一条格式错误丢掉全部抽取。
///
/// 参数:
/// - `raw`: 模型输出原文
/// - `workspace`: 当前工作区路径，用于还原 project 作用域
///
/// 返回:
/// - 归一化后的候选列表；无有效候选时为空
pub fn parse_extraction_output(raw: &str, workspace: Option<&str>) -> Result<Vec<MemoryCandidate>> {
    let Some(json) = extract_json_object(raw) else {
        return Ok(Vec::new());
    };
    let value: Value = serde_json::from_str(json)?;
    let Some(entries) = value.get("memories").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    Ok(entries
        .iter()
        .filter_map(|entry| parse_candidate(entry, workspace))
        .collect())
}

/// 解析单条候选。
///
/// 参数:
/// - `entry`: 候选 JSON 对象
/// - `workspace`: 当前工作区路径
///
/// 返回:
/// - 归一化候选；字段缺失或非法时为 None
fn parse_candidate(entry: &Value, workspace: Option<&str>) -> Option<MemoryCandidate> {
    let kind = MemoryKind::parse(entry.get("kind")?.as_str()?)?;
    let content = entry.get("content")?.as_str()?.to_string();
    let salience = entry.get("salience").and_then(Value::as_f64).unwrap_or(0.0);
    let scope = match entry.get("scope").and_then(Value::as_str) {
        // 模型说 project 但当前没有工作区时降级为全局，避免写入无法匹配的作用域
        Some("project") => MemoryScope::from_stored(workspace),
        _ => MemoryScope::Global,
    };
    let tags = entry
        .get("tags")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    MemoryCandidate {
        kind,
        scope,
        content,
        salience,
        tags,
    }
    .normalized()
}

/// 从可能带有前后文的文本中截出第一个 JSON 对象。
///
/// 参数:
/// - `content`: 模型输出原文
///
/// 返回:
/// - JSON 对象文本；找不到完整对象时为 None
fn extract_json_object(content: &str) -> Option<&str> {
    let trimmed = content.trim();
    let start = trimmed.find('{')?;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, ch) in trimmed[start..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && in_string {
            escaped = true;
            continue;
        }
        if ch == '"' {
            in_string = !in_string;
            continue;
        }
        if in_string {
            continue;
        }
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(&trimmed[start..start + offset + ch.len_utf8()]);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证解析出完整字段的候选。
    #[test]
    fn parses_a_complete_candidate() {
        let raw = r#"{"memories":[{"kind":"preference","content":"用户一律使用 pnpm","salience":0.8,"scope":"global","tags":["包管理"]}]}"#;
        let candidates = parse_extraction_output(raw, None).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].kind, MemoryKind::Preference);
        assert_eq!(candidates[0].content, "用户一律使用 pnpm");
        assert_eq!(candidates[0].salience, 0.8);
        assert_eq!(candidates[0].scope, MemoryScope::Global);
        assert_eq!(candidates[0].tags, vec!["包管理".to_string()]);
    }

    /// 验证 project 作用域绑定到当前工作区。
    #[test]
    fn project_scope_binds_to_the_current_workspace() {
        let raw = r#"{"memories":[{"kind":"decision","content":"构建改用 vite","salience":0.9,"scope":"project"}]}"#;
        let candidates = parse_extraction_output(raw, Some("/home/a")).unwrap();
        assert_eq!(
            candidates[0].scope,
            MemoryScope::from_stored(Some("/home/a"))
        );
    }

    /// 验证没有工作区时 project 作用域降级为全局。
    ///
    /// 否则会写入一条永远无法命中的记忆。
    #[test]
    fn project_scope_falls_back_to_global_without_a_workspace() {
        let raw = r#"{"memories":[{"kind":"decision","content":"构建改用 vite","salience":0.9,"scope":"project"}]}"#;
        let candidates = parse_extraction_output(raw, None).unwrap();
        assert_eq!(candidates[0].scope, MemoryScope::Global);
    }

    /// 验证空抽取结果被正确解析。
    #[test]
    fn parses_an_empty_extraction() {
        let candidates = parse_extraction_output(r#"{"memories":[]}"#, None).unwrap();
        assert!(candidates.is_empty());
    }

    /// 验证 JSON 前后的解释文字不影响解析。
    #[test]
    fn tolerates_prose_around_the_json() {
        let raw = "这轮对话的抽取结果：\n```json\n{\"memories\":[{\"kind\":\"fact\",\"content\":\"用户使用 Arch Linux\",\"salience\":0.7}]}\n```\n以上。";
        let candidates = parse_extraction_output(raw, None).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].content, "用户使用 Arch Linux");
    }

    /// 验证单条格式错误不会丢掉同批次的其它候选。
    #[test]
    fn a_malformed_entry_does_not_discard_the_batch() {
        let raw = r#"{"memories":[{"kind":"nonsense","content":"x"},{"kind":"fact","content":"用户使用 Arch Linux","salience":0.7}]}"#;
        let candidates = parse_extraction_output(raw, None).unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].content, "用户使用 Arch Linux");
    }

    /// 验证完全没有 JSON 时返回空而不是报错。
    #[test]
    fn non_json_output_yields_no_candidates() {
        let candidates = parse_extraction_output("这轮没有值得记住的内容。", None).unwrap();
        assert!(candidates.is_empty());
    }

    /// 验证抽取输入包含工作区信息。
    #[test]
    fn extraction_input_carries_the_workspace() {
        let input = build_extraction_input("问题", "回答", Some("/home/a"));
        assert!(input.contains("当前工作区：/home/a"));
        assert!(input.contains("问题"));
        assert!(input.contains("回答"));
    }
}
