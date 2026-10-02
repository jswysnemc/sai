pub(crate) mod store;

use crate::question::{QuestionOption, QuestionPrompt, QuestionRequest};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

pub(crate) const MAX_PLAN_BYTES: usize = 100_000;
pub(crate) const APPROVE: &str = "Approve and implement";

/// 【计划模式】【会话记录】计划正文与审批状态的唯一持久化记录。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct PlanRecord {
    pub title: String,
    pub plan: String,
    pub status: String,
    pub execution_mode: String,
    pub feedback: Option<String>,
}

/// 【计划模式】【提交参数】模型提交的完整计划，路径由会话管理，不允许指定任意文件。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlanSubmission {
    pub title: String,
    pub plan: String,
}

impl PlanSubmission {
    /// 【计划模式】【参数校验】参数为工具 JSON，返回去除首尾空白的合法计划。
    pub fn parse(arguments: &str) -> Result<Self> {
        let mut value: Self = serde_json::from_str(arguments)?;
        value.title = value.title.trim().to_string();
        value.plan = value.plan.trim().to_string();
        if value.title.is_empty()
            || value.title.chars().count() > 80
            || value.title.chars().any(char::is_control)
        {
            bail!("plan title must contain 1–80 characters without control characters");
        }
        if value.plan.is_empty()
            || value.plan.len() > MAX_PLAN_BYTES
            || value
                .plan
                .chars()
                .any(|ch| ch.is_control() && !matches!(ch, '\n' | '\r' | '\t'))
        {
            bail!("plan must contain 1–100000 bytes without terminal control characters");
        }
        Ok(value)
    }
}

/// 【计划模式】【审批表单】参数为批准后恢复的模式，返回无默认答案的审批问题。
pub(crate) fn approval_request(mode: &str) -> QuestionRequest {
    QuestionRequest { questions: vec![QuestionPrompt {
        header: "Plan review".into(),
        question: format!("Review the full plan. Approve to start implementation in {mode} mode, or provide changes to keep planning."),
        options: vec![
            QuestionOption { label: "Approve and implement".into(), description: format!("Exit Plan mode and implement the reviewed plan in {mode} mode."), value: Some(APPROVE.into()) },
            QuestionOption { label: "Keep planning".into(), description: "Stay in read-only Plan mode and revise the approach.".into(), value: Some("Keep planning".into()) },
        ],
        multiple: false, custom: true, required: true, default_answers: vec![], validation: None,
    }] }
}
