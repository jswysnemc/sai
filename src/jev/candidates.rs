/// 候选资源类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CandidateKind {
    /// 延迟暴露的工具
    Tool,
    /// 已安装的 skill
    Skill,
    /// 按需暴露的提示词或记忆上下文
    Prompt,
}

impl CandidateKind {
    /// 返回提交给 Jev 的类型文本。
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Tool => "tool",
            Self::Skill => "skill",
            Self::Prompt => "prompt",
        }
    }
}

/// 一个尚未暴露给模型、等待 Jev 判断的资源。
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Candidate {
    /// 资源类型
    pub kind: CandidateKind,
    /// 工具名或 skill 名
    pub name: String,
    /// 资源简介
    pub description: String,
}

/// 候选描述提交给 Jev 时的最大字符数。
const MAX_DESCRIPTION_CHARS: usize = 400;

impl Candidate {
    /// 构造候选并截断过长简介。
    ///
    /// 参数:
    /// - `kind`: 资源类型
    /// - `name`: 资源名
    /// - `description`: 原始简介
    ///
    /// 返回:
    /// - 候选资源
    pub(crate) fn new(kind: CandidateKind, name: impl Into<String>, description: &str) -> Self {
        let description = description.split_whitespace().collect::<Vec<_>>().join(" ");
        let description = if description.chars().count() > MAX_DESCRIPTION_CHARS {
            let clipped: String = description.chars().take(MAX_DESCRIPTION_CHARS).collect();
            format!("{clipped}…")
        } else {
            description
        };
        Self {
            kind,
            name: name.into(),
            description,
        }
    }
}

/// 从全部资源中剔除已经暴露的项，得到本次需要 Jev 判断的候选列表。
///
/// 参数:
/// - `all`: 当前 Agent 可加载的全部工具与 skill
/// - `is_exposed`: 判断资源是否已经暴露给模型
///
/// 返回:
/// - 保持原顺序的未暴露候选
pub(crate) fn pending_candidates(
    all: Vec<Candidate>,
    is_exposed: impl Fn(&Candidate) -> bool,
) -> Vec<Candidate> {
    all.into_iter()
        .filter(|candidate| !is_exposed(candidate))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposed_candidates_are_removed_from_judgement_list() {
        let all = vec![
            Candidate::new(CandidateKind::Tool, "web_search", "Search the web."),
            Candidate::new(CandidateKind::Skill, "drawio", "Draw diagrams."),
            Candidate::new(CandidateKind::Tool, "web_fetch", "Fetch a URL."),
        ];
        let pending = pending_candidates(all, |candidate| candidate.name == "web_search");
        let names = pending
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, ["drawio", "web_fetch"]);
    }

    #[test]
    fn long_description_is_clipped_and_whitespace_collapsed() {
        let text = format!("a\n\n  b {}", "x".repeat(1_000));
        let candidate = Candidate::new(CandidateKind::Skill, "s", &text);
        assert!(candidate.description.starts_with("a b "));
        assert!(candidate.description.chars().count() <= MAX_DESCRIPTION_CHARS + 1);
    }
}
