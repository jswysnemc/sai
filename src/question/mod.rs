mod broker;

pub(crate) use broker::{
    answer_question, cancel_question, discard_pending_questions_for_session, pending_questions,
    request_question, resolve_question, PendingQuestion,
};

use anyhow::{bail, Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

const MAX_QUESTIONS: usize = 8;
const MAX_OPTIONS: usize = 8;
const MAX_HEADER_CHARS: usize = 30;
const MAX_QUESTION_CHARS: usize = 1_000;
const MAX_LABEL_CHARS: usize = 80;
const MAX_DESCRIPTION_CHARS: usize = 400;
pub const MAX_CUSTOM_ANSWER_CHARS: usize = 4_000;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct QuestionOption {
    pub label: String,
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

impl QuestionOption {
    /// 返回提交给请求方的真实选项值。
    ///
    /// 返回:
    /// - 显式值存在时返回该值，否则回退到展示名称
    pub fn answer_value(&self) -> &str {
        self.value.as_deref().unwrap_or(&self.label)
    }
}

/// 结构化问题对自定义答案施加的基础约束。
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct QuestionValidation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimum: Option<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub maximum: Option<serde_json::Number>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_length: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_length: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct QuestionPrompt {
    pub header: String,
    pub question: String,
    pub options: Vec<QuestionOption>,
    #[serde(default)]
    pub multiple: bool,
    #[serde(default = "default_true")]
    pub custom: bool,
    #[serde(default = "default_true")]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub default_answers: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation: Option<QuestionValidation>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct QuestionRequest {
    pub questions: Vec<QuestionPrompt>,
}

pub type QuestionAnswers = Vec<Vec<String>>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", content = "data", rename_all = "snake_case")]
pub enum QuestionResponse {
    Answered(QuestionAnswers),
    Cancelled,
    Unavailable(String),
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct QuestionCancelled;

impl fmt::Display for QuestionCancelled {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("question cancelled by user")
    }
}

impl Error for QuestionCancelled {}

#[allow(dead_code)]
pub fn is_question_cancelled(error: &anyhow::Error) -> bool {
    error.downcast_ref::<QuestionCancelled>().is_some()
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct QuestionExchange {
    pub questions: Vec<QuestionPrompt>,
    pub answers: QuestionAnswers,
    pub answered_at: String,
}

impl QuestionExchange {
    pub fn new(request: QuestionRequest, answers: QuestionAnswers) -> Result<Self> {
        request.validate()?;
        validate_answers(&request, &answers)?;
        Ok(Self {
            questions: request.questions,
            answers,
            answered_at: Utc::now().to_rfc3339(),
        })
    }
}

impl QuestionRequest {
    pub fn parse(arguments: &str) -> Result<Self> {
        let request: Self = serde_json::from_str(arguments)?;
        request.validate()?;
        Ok(request)
    }

    pub fn validate(&self) -> Result<()> {
        if self.questions.is_empty() {
            bail!("questions must contain at least one question");
        }
        if self.questions.len() > MAX_QUESTIONS {
            bail!("questions cannot contain more than {MAX_QUESTIONS} questions");
        }
        for (question_index, question) in self.questions.iter().enumerate() {
            validate_text(&question.header, "header", question_index, MAX_HEADER_CHARS)?;
            validate_text(
                &question.question,
                "question",
                question_index,
                MAX_QUESTION_CHARS,
            )?;
            if question.options.len() > MAX_OPTIONS {
                bail!(
                    "questions[{question_index}].options cannot contain more than {MAX_OPTIONS} options"
                );
            }
            if question.options.is_empty() && !question.custom {
                bail!(
                    "questions[{question_index}] must provide an option or allow a custom answer"
                );
            }
            let mut labels = BTreeSet::new();
            let mut values = BTreeSet::new();
            for (option_index, option) in question.options.iter().enumerate() {
                validate_text(
                    &option.label,
                    &format!("options[{option_index}].label"),
                    question_index,
                    MAX_LABEL_CHARS,
                )?;
                if option.description.chars().any(char::is_control) {
                    bail!(
                        "questions[{question_index}].options[{option_index}].description contains control characters"
                    );
                }
                if option.description.chars().count() > MAX_DESCRIPTION_CHARS {
                    bail!(
                        "questions[{question_index}].options[{option_index}].description is too long"
                    );
                }
                if !labels.insert(option.label.trim()) {
                    bail!("questions[{question_index}] contains duplicate option labels");
                }
                let value = option.answer_value();
                validate_text(value, "option value", question_index, MAX_LABEL_CHARS)?;
                if !values.insert(value) {
                    bail!("questions[{question_index}] contains duplicate option values");
                }
            }
            validate_question_constraints(question, question_index)?;
            validate_question_answer(question, &question.default_answers, question_index)?;
        }
        Ok(())
    }

    pub fn needs_review(&self) -> bool {
        self.questions.len() > 1
            || self
                .questions
                .iter()
                .any(|question| question.multiple || !question.required)
    }
}

pub fn validate_answers(request: &QuestionRequest, answers: &QuestionAnswers) -> Result<()> {
    if answers.len() != request.questions.len() {
        bail!("answer count does not match question count");
    }
    for (index, (question, answer)) in request.questions.iter().zip(answers).enumerate() {
        if answer.is_empty() && question.required {
            bail!("question {index} is unanswered");
        }
        if answer.is_empty() {
            continue;
        }
        if !question.multiple && answer.len() != 1 {
            bail!("question {index} only accepts one answer");
        }
        let option_labels = question
            .options
            .iter()
            .map(QuestionOption::answer_value)
            .collect::<BTreeSet<_>>();
        let mut unique = BTreeSet::new();
        for value in answer {
            let value = value.trim();
            if value.is_empty() {
                bail!("question {index} contains an empty answer");
            }
            if value.chars().count() > MAX_CUSTOM_ANSWER_CHARS {
                bail!("question {index} contains an answer that is too long");
            }
            if !option_labels.contains(value) && !question.custom {
                bail!("question {index} does not allow custom answers");
            }
            if !unique.insert(value) {
                bail!("question {index} contains duplicate answers");
            }
        }
        validate_question_answer(question, answer, index)?;
    }
    Ok(())
}

/// 校验问题声明的约束自身是否有效。
///
/// 参数:
/// - `question`: 待校验问题
/// - `index`: 问题序号
///
/// 返回:
/// - 约束有效时返回空结果
fn validate_question_constraints(question: &QuestionPrompt, index: usize) -> Result<()> {
    let Some(validation) = &question.validation else {
        return Ok(());
    };
    if validation
        .minimum
        .as_ref()
        .and_then(serde_json::Number::as_f64)
        .zip(
            validation
                .maximum
                .as_ref()
                .and_then(serde_json::Number::as_f64),
        )
        .is_some_and(|(min, max)| min > max)
    {
        bail!("question {index} minimum exceeds maximum");
    }
    if validation
        .min_length
        .zip(validation.max_length)
        .is_some_and(|(min, max)| min > max)
    {
        bail!("question {index} min_length exceeds max_length");
    }
    if let Some(pattern) = &validation.pattern {
        regex::Regex::new(pattern)
            .with_context(|| format!("question {index} contains an invalid pattern"))?;
    }
    Ok(())
}

/// 按问题类型和范围校验一组答案。
///
/// 参数:
/// - `question`: 约束来源
/// - `answers`: 待校验答案
/// - `index`: 问题序号
///
/// 返回:
/// - 所有答案满足约束时返回空结果
fn validate_question_answer(
    question: &QuestionPrompt,
    answers: &[String],
    index: usize,
) -> Result<()> {
    let Some(validation) = &question.validation else {
        return Ok(());
    };
    for answer in answers {
        let length = answer.chars().count();
        if validation
            .min_length
            .is_some_and(|minimum| length < minimum)
        {
            bail!("question {index} answer is shorter than the minimum length");
        }
        if validation
            .max_length
            .is_some_and(|maximum| length > maximum)
        {
            bail!("question {index} answer exceeds the maximum length");
        }
        if let Some(pattern) = &validation.pattern {
            if !regex::Regex::new(pattern)?.is_match(answer) {
                bail!("question {index} answer does not match the required pattern");
            }
        }
        validate_typed_answer(validation, answer, index)?;
    }
    Ok(())
}

/// 校验单个答案的标量类型、数值范围和常见格式。
///
/// 参数:
/// - `validation`: 问题约束
/// - `answer`: 待校验答案
/// - `index`: 问题序号
///
/// 返回:
/// - 答案满足类型与格式时返回空结果
fn validate_typed_answer(
    validation: &QuestionValidation,
    answer: &str,
    index: usize,
) -> Result<()> {
    match validation.value_type.as_deref() {
        Some("integer") => {
            let value = answer
                .parse::<i64>()
                .with_context(|| format!("question {index} requires an integer"))?
                as f64;
            validate_number_range(validation, value, index)?;
        }
        Some("number") => {
            let value = answer
                .parse::<f64>()
                .with_context(|| format!("question {index} requires a number"))?;
            validate_number_range(validation, value, index)?;
        }
        Some("boolean") => {
            answer
                .parse::<bool>()
                .with_context(|| format!("question {index} requires a boolean"))?;
        }
        _ => {}
    }
    match validation.format.as_deref() {
        Some("date") => {
            chrono::NaiveDate::parse_from_str(answer, "%Y-%m-%d")
                .with_context(|| format!("question {index} requires an ISO date"))?;
        }
        Some("date-time") => {
            chrono::DateTime::parse_from_rfc3339(answer)
                .with_context(|| format!("question {index} requires an ISO date-time"))?;
        }
        Some("uri" | "url") => {
            reqwest::Url::parse(answer)
                .with_context(|| format!("question {index} requires an absolute URI"))?;
        }
        Some("ipv4") => {
            answer
                .parse::<std::net::Ipv4Addr>()
                .with_context(|| format!("question {index} requires an IPv4 address"))?;
        }
        Some("ipv6") => {
            answer
                .parse::<std::net::Ipv6Addr>()
                .with_context(|| format!("question {index} requires an IPv6 address"))?;
        }
        Some("email") if !answer.contains('@') => {
            bail!("question {index} requires an email address");
        }
        _ => {}
    }
    Ok(())
}

/// 校验数值答案的上下界。
///
/// 参数:
/// - `validation`: 数值约束
/// - `value`: 已解析数值
/// - `index`: 问题序号
///
/// 返回:
/// - 数值位于范围内时返回空结果
fn validate_number_range(validation: &QuestionValidation, value: f64, index: usize) -> Result<()> {
    if validation
        .minimum
        .as_ref()
        .and_then(serde_json::Number::as_f64)
        .is_some_and(|minimum| value < minimum)
    {
        bail!("question {index} answer is below the minimum");
    }
    if validation
        .maximum
        .as_ref()
        .and_then(serde_json::Number::as_f64)
        .is_some_and(|maximum| value > maximum)
    {
        bail!("question {index} answer exceeds the maximum");
    }
    Ok(())
}

pub fn answered_tool_output(exchange: &QuestionExchange) -> String {
    let answers = exchange
        .questions
        .iter()
        .zip(&exchange.answers)
        .map(|(question, selected)| {
            // 单选时直接返回字符串，多选时返回数组
            let answer_value = if question.multiple {
                json!(selected)
            } else {
                json!(selected.first().cloned().unwrap_or_default())
            };
            json!({
                "header": question.header,
                "question": question.question,
                "answer": answer_value,
            })
        })
        .collect::<Vec<_>>();
    serde_json::to_string(&json!({
        "status": "answered",
        "answers": answers,
        "instruction": "Continue using these user-provided answers. Do not ask the same questions again.",
    }))
    .unwrap_or_else(|_| "{\"status\":\"answered\"}".to_string())
}

pub fn unavailable_tool_output(reason: &str) -> String {
    serde_json::to_string(&json!({
        "status": "unavailable",
        "reason": reason,
        "instruction": "Interactive input is unavailable. Continue safely without assuming an answer.",
    }))
    .unwrap_or_else(|_| "{\"status\":\"unavailable\"}".to_string())
}

#[allow(dead_code)]
pub fn assistant_exchange_text(exchange: &QuestionExchange) -> String {
    let mut output = String::from("补充确认：");
    for (index, question) in exchange.questions.iter().enumerate() {
        output.push_str(&format!(
            "\n{}. [{}] {}",
            index + 1,
            question.header,
            question.question.trim()
        ));
        for option in &question.options {
            output.push_str(&format!("\n   - {}", option.label));
            if !option.description.is_empty() {
                output.push_str(&format!(": {}", option.description));
            }
        }
        if question.custom {
            output.push_str("\n   - 可输入自定义答案");
        }
    }
    output
}

#[allow(dead_code)]
pub fn user_exchange_text(exchange: &QuestionExchange) -> String {
    let mut output = String::from("补充回答：");
    for (question, answers) in exchange.questions.iter().zip(&exchange.answers) {
        output.push_str(&format!("\n- {}：{}", question.header, answers.join("、")));
    }
    output
}

fn validate_text(value: &str, field: &str, question_index: usize, max_chars: usize) -> Result<()> {
    if value.trim().is_empty() {
        bail!("questions[{question_index}].{field} cannot be empty");
    }
    if value.trim() != value {
        bail!("questions[{question_index}].{field} cannot have surrounding whitespace");
    }
    if value.chars().any(char::is_control) {
        bail!("questions[{question_index}].{field} contains control characters");
    }
    if value.chars().count() > max_chars {
        bail!("questions[{question_index}].{field} is too long");
    }
    Ok(())
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> QuestionRequest {
        QuestionRequest {
            questions: vec![QuestionPrompt {
                header: "范围".to_string(),
                question: "修改哪些文件？".to_string(),
                options: vec![QuestionOption {
                    label: "全部".to_string(),
                    description: "修改全部相关文件".to_string(),
                    value: None,
                }],
                multiple: false,
                custom: true,
                required: true,
                default_answers: Vec::new(),
                validation: None,
            }],
        }
    }

    #[test]
    fn request_accepts_option_and_custom_answers() {
        let request = request();
        assert!(validate_answers(&request, &vec![vec!["全部".to_string()]]).is_ok());
        assert!(validate_answers(&request, &vec![vec!["仅配置".to_string()]]).is_ok());
    }

    #[test]
    fn single_question_rejects_multiple_answers() {
        let request = request();
        assert!(validate_answers(
            &request,
            &vec![vec!["全部".to_string(), "仅配置".to_string()]]
        )
        .is_err());
    }

    #[test]
    fn request_rejects_duplicate_labels() {
        let mut request = request();
        let duplicate = request.questions[0].options[0].clone();
        request.questions[0].options.push(duplicate);
        assert!(request.validate().is_err());
    }

    #[test]
    fn request_rejects_terminal_control_sequences() {
        let mut request = request();
        request.questions[0].question = "选择\u{1b}[2J范围".to_string();
        assert!(request.validate().is_err());
    }

    #[test]
    fn request_rejects_surrounding_label_whitespace() {
        let mut request = request();
        request.questions[0].options[0].label = " 全部 ".to_string();
        assert!(request.validate().is_err());
    }

    #[test]
    fn persisted_assistant_exchange_keeps_option_meaning() {
        let exchange = QuestionExchange::new(request(), vec![vec!["全部".to_string()]]).unwrap();
        let text = assistant_exchange_text(&exchange);
        assert!(text.contains("全部: 修改全部相关文件"));
        assert!(text.contains("可输入自定义答案"));
    }

    /// 选项提交值可以与展示名称不同。
    #[test]
    fn validates_explicit_option_values() {
        let mut request = request();
        request.questions[0].custom = false;
        request.questions[0].options[0].value = Some("all".to_string());

        assert!(validate_answers(&request, &vec![vec!["all".to_string()]]).is_ok());
        assert!(validate_answers(&request, &vec![vec!["全部".to_string()]]).is_err());
    }
}
