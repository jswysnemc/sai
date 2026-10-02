use super::model::ToolView;
use crate::question::{QuestionPrompt, QuestionRequest};
use crate::render::activity_animation::render_activity_line;
use crate::render::status_style::ToolHealth;
use crate::render::tool_event_line::tool_status_line;
use crate::render::ToolCallDisplayMode;
use serde_json::Value;

/// 【提问展示】【专用卡片】参数为工具视图、展示模式与动画帧，返回问题和答案，不输出协议载荷。
pub(super) fn render(view: &ToolView, mode: ToolCallDisplayMode, frame: usize) -> String {
    let request = serde_json::from_str::<QuestionRequest>(&view.arguments).ok();
    let response = view
        .outcome
        .as_ref()
        .and_then(|outcome| serde_json::from_str::<Value>(&outcome.output).ok());
    let answers = response
        .as_ref()
        .and_then(|value| value["answers"].as_array());
    let count = request
        .as_ref()
        .map(|request| request.questions.len())
        .or_else(|| answers.map(Vec::len))
        .unwrap_or(0);
    let title = match (view.outcome.is_some(), count) {
        (false, 0) => "Asking".into(),
        (true, 0) => "Asked".into(),
        (done, count) => format!(
            "{} {count} {}",
            if done { "Asked" } else { "Asking" },
            if count == 1 { "question" } else { "questions" }
        ),
    };
    let health = match &view.outcome {
        None => ToolHealth::Pending,
        Some(outcome) if outcome.ok => ToolHealth::Ok,
        Some(_) => ToolHealth::Neutral,
    };
    let mut lines = vec![if view.outcome.is_none() {
        render_activity_line(&title, "", frame)
    } else {
        tool_status_line(&title, "", health)
    }];
    if view.outcome.is_none() && mode == ToolCallDisplayMode::Summary {
        return lines.join("\n");
    }
    // 1. 【提问展示】【问题配对】优先保留请求的选项标签；历史仅剩结果时读取结果中的问题正文
    for index in 0..count {
        let question = request
            .as_ref()
            .and_then(|request| request.questions.get(index));
        let answer = answers.and_then(|answers| answers.get(index));
        let header = question
            .map(|question| question.header.as_str())
            .or_else(|| answer.and_then(|value| value["header"].as_str()))
            .unwrap_or("");
        let body = question
            .map(|question| question.question.as_str())
            .or_else(|| answer.and_then(|value| value["question"].as_str()))
            .unwrap_or("");
        let selected = answer_values(answer);
        if !header.is_empty() {
            lines.push(format!("  \x1b[1m{}\x1b[0m", clean(header)));
        }
        push_text(&mut lines, "  ", body, "");
        if mode == ToolCallDisplayMode::Full {
            if let Some(question) = question {
                for (option_index, option) in question.options.iter().enumerate() {
                    let picked = selected.iter().any(|value| value == option.answer_value());
                    let marker = match (question.multiple, picked) {
                        (false, false) => "○",
                        (false, true) => "●",
                        (true, false) => "□",
                        (true, true) => "■",
                    };
                    let style = if picked { "\x1b[36m" } else { "\x1b[2m" };
                    push_text(
                        &mut lines,
                        "    ",
                        &format!("{marker} {}. {}", option_index + 1, option.label),
                        style,
                    );
                    if !option.description.is_empty() {
                        push_text(&mut lines, "      ", &option.description, "\x1b[2m");
                    }
                }
            }
        }
        for value in selected {
            let label = answer_label(question, &value);
            push_text(&mut lines, "  › ", &label, "\x1b[36m");
        }
        if let Some(count) = answer
            .and_then(|value| value["image_count"].as_u64())
            .filter(|count| *count > 0)
        {
            lines.push(format!("    \x1b[2m{count} attached images\x1b[0m"));
        }
    }
    // 2. 【提问展示】【结束原因】仅展示面向用户的原因，模型继续指令保持在协议内部
    if let Some(outcome) = &view.outcome {
        if !outcome.ok {
            let reason = response
                .as_ref()
                .and_then(|value| value["reason"].as_str())
                .unwrap_or_else(|| {
                    if response.is_some() {
                        "Question unavailable"
                    } else {
                        &outcome.output
                    }
                });
            push_text(&mut lines, "  ", reason, "\x1b[2m");
        } else if answers.is_none() {
            push_text(&mut lines, "  ", "No answer recorded", "\x1b[2m");
        }
    }
    lines.join("\n")
}

/// 【提问展示】【答案解析】参数为单题结果，返回单选或多选的答案值。
fn answer_values(answer: Option<&Value>) -> Vec<String> {
    match answer.map(|value| &value["answer"]) {
        Some(Value::String(value)) if !value.is_empty() => vec![value.clone()],
        Some(Value::Array(values)) => values
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

/// 【提问展示】【选项标签】参数为原问题与答案值，返回展示标签或自定义原文。
fn answer_label(question: Option<&QuestionPrompt>, value: &str) -> String {
    question
        .and_then(|question| {
            question
                .options
                .iter()
                .find(|option| option.answer_value() == value)
        })
        .map(|option| option.label.clone())
        .unwrap_or_else(|| value.to_string())
}

/// 【提问展示】【多行文本】参数为输出行、前缀、正文和样式；逐行追加安全文本，无返回值。
fn push_text(lines: &mut Vec<String>, prefix: &str, text: &str, style: &str) {
    for line in clean(text).lines() {
        lines.push(format!("{prefix}{style}{line}\x1b[0m"));
    }
}

/// 【提问展示】【控制字符】参数为用户或工具文本，返回保留换行的可打印内容。
fn clean(text: &str) -> String {
    text.chars()
        .filter(|ch| !ch.is_control() || matches!(ch, '\n' | '\t'))
        .collect()
}
