use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

/// 【终端】【技能匹配】按名称与描述模糊匹配技能，并按相关度返回全部结果
///
/// 参数:
/// - `skills`: 原始技能目录，目录顺序用于空查询和同分结果
/// - `query`: 查询词，忽略首尾空白和大小写
///
/// 返回:
/// - 名称完全匹配、名称模糊匹配、描述匹配依次排列的技能引用
pub(super) fn matching_skills<'a>(
    skills: &'a [(String, String)],
    query: &str,
) -> Vec<&'a (String, String)> {
    let query = query.trim();
    if query.is_empty() {
        return skills.iter().collect();
    }
    let matcher = SkimMatcherV2::default().ignore_case();
    // 1. 【终端】【技能匹配】优先匹配名称，名称未命中时才使用描述得分
    let mut matches = skills
        .iter()
        .filter_map(|skill| match_rank(&matcher, skill, query).map(|rank| (rank, skill)))
        .collect::<Vec<_>>();
    // 2. 【终端】【技能匹配】按匹配层级和得分降序排列，稳定排序保留同分项顺序
    matches.sort_by(|left, right| right.0.cmp(&left.0));
    matches.into_iter().map(|(_, skill)| skill).collect()
}

/// 【终端】【技能匹配】计算单个技能的匹配层级与模糊匹配得分
///
/// 参数:
/// - `matcher`: 忽略大小写的模糊匹配器
/// - `skill`: 技能名称与描述
/// - `query`: 非空查询词
///
/// 返回:
/// - 匹配层级与得分，数值越大越优先；未匹配时返回空
fn match_rank(matcher: &SkimMatcherV2, skill: &(String, String), query: &str) -> Option<(u8, i64)> {
    let (name, description) = skill;
    if let Some(score) = matcher.fuzzy_match(name, query) {
        let priority = if name.eq_ignore_ascii_case(query) {
            2
        } else {
            1
        };
        return Some((priority, score));
    }
    matcher
        .fuzzy_match(description, query)
        .map(|score| (0, score))
}
