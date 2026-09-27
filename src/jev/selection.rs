use super::candidates::{Candidate, CandidateKind};
use super::client::NoulQuestion;
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Jev 判断后的暴露结果。
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Selection {
    /// 选中的工具名，按概率降序
    pub tools: Vec<String>,
    /// 选中的 skill 名，按概率降序
    pub skills: Vec<String>,
}

impl Selection {
    /// 判断是否未选中任何资源。
    pub(crate) fn is_empty(&self) -> bool {
        self.tools.is_empty() && self.skills.is_empty()
    }
}

/// 筛选参数。
#[derive(Debug, Clone, Copy)]
pub(crate) struct SelectionLimits {
    /// 最低概率
    pub threshold: f64,
    /// 工具上限
    pub max_tools: usize,
    /// skill 上限
    pub max_skills: usize,
}

/// 构造所有问题共享的判断依据。
///
/// 参数:
/// - `need`: 用户本轮请求或模型描述的能力需求
/// - `conversation`: 近期对话摘要
/// - `available`: 模型已经可以直接使用的工具与 skill 名称
///
/// 返回:
/// - systemone `state`
pub(crate) fn build_state(need: &str, conversation: &str, available: &[String]) -> Value {
    json!({
        "request": need,
        "recent_conversation": conversation,
        "already_available": available,
    })
}

/// 为每个候选构造一个 Noul 问题；问题 id 为候选下标。
///
/// 参数:
/// - `candidates`: 待判断的候选
///
/// 返回:
/// - 问题 id 到问题的映射
pub(crate) fn build_questions(candidates: &[Candidate]) -> BTreeMap<String, NoulQuestion> {
    candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| (question_id(index), question_for(candidate)))
        .collect()
}

/// 按概率、阈值和类型上限筛选候选。
///
/// 参数:
/// - `candidates`: 与问题同序的候选
/// - `answers`: 问题 id 到"是"概率的映射
/// - `limits`: 阈值与上限
///
/// 返回:
/// - 选中的工具与 skill
pub(crate) fn select(
    candidates: &[Candidate],
    answers: &BTreeMap<String, f64>,
    limits: SelectionLimits,
) -> Selection {
    // 1. 过滤低于阈值的候选，并按概率降序排列
    let mut scored = candidates
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            let probability = *answers.get(&question_id(index))?;
            (probability >= limits.threshold).then_some((probability, candidate))
        })
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| right.0.total_cmp(&left.0));
    // 2. 按类型分别截取上限
    let mut selection = Selection::default();
    for (_, candidate) in scored {
        match candidate.kind {
            CandidateKind::Tool if selection.tools.len() < limits.max_tools => {
                selection.tools.push(candidate.name.clone());
            }
            CandidateKind::Skill if selection.skills.len() < limits.max_skills => {
                selection.skills.push(candidate.name.clone());
            }
            _ => {}
        }
    }
    selection
}

/// 生成候选对应的问题 id；id 只供代码使用，不会发送给模型推理。
pub(crate) fn question_id(index: usize) -> String {
    format!("c{index}")
}

/// 构造单个候选的 Noul 问题。
fn question_for(candidate: &Candidate) -> NoulQuestion {
    NoulQuestion {
        instructions: json!({
            "capability": {
                "kind": candidate.kind.as_str(),
                "name": candidate.name,
                "description": candidate.description,
            },
            "question": "An AI coding assistant is handling `request`, with `recent_conversation` as background. It can already use every item in `already_available`. Will the assistant need `capability` to carry out `request`?",
        }),
        criteria_true: "The capability is directly useful for this request: the task clearly calls for what it does, or the request explicitly names it.".to_string(),
        criteria_false: "The capability is unrelated, only loosely related, or the request can be handled by conversation alone or by the already available items.".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidates() -> Vec<Candidate> {
        vec![
            Candidate::new(CandidateKind::Tool, "web_search", "Search the web."),
            Candidate::new(CandidateKind::Tool, "web_fetch", "Fetch a URL."),
            Candidate::new(CandidateKind::Skill, "drawio", "Draw diagrams."),
            Candidate::new(CandidateKind::Skill, "remotion", "Make videos."),
        ]
    }

    #[test]
    fn questions_follow_candidate_order() {
        let questions = build_questions(&candidates());
        assert_eq!(questions.len(), 4);
        assert_eq!(questions["c2"].instructions["capability"]["name"], "drawio");
        assert_eq!(questions["c2"].instructions["capability"]["kind"], "skill");
    }

    #[test]
    fn selection_applies_threshold_order_and_limits() {
        let answers = BTreeMap::from([
            ("c0".to_string(), 0.6),
            ("c1".to_string(), 0.9),
            ("c2".to_string(), 0.4),
            ("c3".to_string(), 0.95),
        ]);
        let limits = SelectionLimits {
            threshold: 0.5,
            max_tools: 1,
            max_skills: 3,
        };
        let selection = select(&candidates(), &answers, limits);
        assert_eq!(selection.tools, ["web_fetch"]);
        assert_eq!(selection.skills, ["remotion"]);
    }

    #[test]
    fn missing_answers_select_nothing() {
        let limits = SelectionLimits {
            threshold: 0.5,
            max_tools: 5,
            max_skills: 5,
        };
        assert!(select(&candidates(), &BTreeMap::new(), limits).is_empty());
    }
}
