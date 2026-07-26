use crate::config::AgentEngineConfig;
use crate::render::terminal_text as t;

/// 渲染当前对话内核的提示行。
///
/// 使用外部内核时，对话的推理与决策已经交给别的 agent，sai 只保留治理与持久化。
/// 这个落差必须说出来：上下文压缩与记忆注入会静默停摆，
/// 用户若不知情，会把它当成故障去排查。
///
/// 参数:
/// - `config`: 内核配置
///
/// 返回:
/// - 外部内核时返回提示文本；原生内核返回 None
pub(crate) fn engine_notice(config: &AgentEngineConfig) -> Option<String> {
    if !config.engine.is_external() {
        return None;
    }
    let label = config.engine.display_label();
    let features = config.engine.unavailable_features().join("、");
    Some(format!(
        "{} {label} · {} {features}",
        t("Engine:", "对话内核："),
        t("unavailable:", "以下功能停用："),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AgentEngineKind;

    /// 原生内核没有落差，不打扰用户。
    #[test]
    fn native_engine_has_no_notice() {
        assert!(engine_notice(&AgentEngineConfig::default()).is_none());
    }

    /// 外部内核必须同时说清「换成了谁」与「失去了什么」。
    #[test]
    fn external_engine_names_the_engine_and_the_gap() {
        let config = AgentEngineConfig {
            engine: AgentEngineKind::Codex,
            ..AgentEngineConfig::default()
        };

        let notice = engine_notice(&config).expect("external engine must be announced");

        assert!(notice.contains("Codex"));
        // 至少要提到压缩：它是最容易被误认为故障的一项
        assert!(
            notice.contains("压缩") || notice.contains("compaction"),
            "notice should mention compaction: {notice}"
        );
    }
}
