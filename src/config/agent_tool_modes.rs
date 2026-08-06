use std::collections::BTreeSet;

/// 延迟集合通配符：白名单内的全部非基础工具都需要 load。
///
/// CLI 等全量开放的 Agent 无法穷举工具名（MCP 工具在运行期才注册），
/// 因此用通配符表达「基础工具直接可见，其余一律延迟」。
pub const DEFERRED_ALL_NON_BASE: &str = "*";

/// 将延迟集合收敛到启用集合内部，并去重排序。
///
/// 白名单收窄后残留的延迟项没有意义，保留下来只会让配置读起来自相矛盾。
/// 通配符不受白名单约束，始终保留。
///
/// 参数:
/// - `enabled`: Agent 可用工具白名单，空表示全量
/// - `deferred`: 待归一化的延迟工具名
///
/// 返回:
/// - 去重且落在白名单内的延迟工具名
pub fn normalize_deferred_tools(enabled: &[String], deferred: &[String]) -> Vec<String> {
    let allowed = enabled.iter().cloned().collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    deferred
        .iter()
        .filter(|name| {
            *name == DEFERRED_ALL_NON_BASE || allowed.is_empty() || allowed.contains(*name)
        })
        .filter(|name| seen.insert((*name).clone()))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证归一化会去重并丢弃白名单外的延迟项。
    #[test]
    fn normalize_drops_out_of_whitelist_and_duplicates() {
        let enabled = vec!["read_file".to_string(), "web_search".to_string()];
        let deferred = vec![
            "web_search".to_string(),
            "web_search".to_string(),
            "show_meme".to_string(),
        ];

        assert_eq!(
            normalize_deferred_tools(&enabled, &deferred),
            vec!["web_search".to_string()]
        );
    }

    /// 验证白名单为空时保留全部延迟项。
    #[test]
    fn normalize_keeps_all_when_whitelist_is_empty() {
        let deferred = vec!["deep_diagnose".to_string(), "show_meme".to_string()];

        assert_eq!(normalize_deferred_tools(&[], &deferred), deferred);
    }

    /// 验证通配符不受白名单收敛影响。
    #[test]
    fn normalize_keeps_wildcard_regardless_of_whitelist() {
        let enabled = vec!["read_file".to_string()];
        let deferred = vec![DEFERRED_ALL_NON_BASE.to_string(), "show_meme".to_string()];

        assert_eq!(
            normalize_deferred_tools(&enabled, &deferred),
            vec![DEFERRED_ALL_NON_BASE.to_string()]
        );
    }
}
