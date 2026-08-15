use super::model::AppConfig;

/// 判断某个工具在当前运行配置下最终能否到达模型。
///
/// 只回答白名单这一层：工具是否注册由插件开关决定，白名单是在注册之后
/// 再做一次收窄，两关都过才算真正可用。语义必须与
/// `runner::submission_tools::apply_enabled_tools_filter` 保持一致，
/// 否则会出现"以为可用其实被过滤掉"的错判。
///
/// 参数:
/// - `config`: 已应用 Agent 覆盖的运行配置
/// - `name`: 工具名
///
/// 返回:
/// - 白名单放行该工具时为 true
pub fn whitelist_allows_tool(config: &AppConfig, name: &str) -> bool {
    let Some(runtime) = config.agent_runtime.as_ref() else {
        return true;
    };
    // 非独占模式下空白名单沿用旧语义：不做收窄
    if runtime.enabled_tools.is_empty() && !runtime.exclusive {
        return true;
    }
    runtime.enabled_tools.iter().any(|tool| tool == name)
}

/// 找出白名单中指向不存在工具的条目。
///
/// 工具改名或下线后，旧配置里的名字会在过滤时被静默丢弃，既不报错也不
/// 提示，只表现为"这个 Agent 少了点什么"。诊断命令据此把死名字点出来。
///
/// 已知的改名不算死名字：迁移时当前注册名已经补进白名单，功能没有缺失，
/// 报出来只会让人以为坏了。
///
/// 参数:
/// - `config`: 已应用 Agent 覆盖的运行配置
/// - `registered`: 当前实际注册的全部工具名
///
/// 返回:
/// - 白名单里既无对应工具、也无改名替代的名字，按配置顺序排列
pub fn unknown_whitelist_tools(config: &AppConfig, registered: &[String]) -> Vec<String> {
    let Some(runtime) = config.agent_runtime.as_ref() else {
        return Vec::new();
    };
    runtime
        .enabled_tools
        .iter()
        .filter(|name| !registered.iter().any(|tool| tool == *name))
        .filter(|name| {
            // 改名后的当前名已经在白名单里，这一条只是残留的旧写法
            match super::agent_presets::legacy_tool_replacement(name) {
                Some(current) => !registered.iter().any(|tool| tool == current),
                None => true,
            }
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AgentRuntimeOverride;

    /// 构造带白名单的运行配置。
    fn config_with_whitelist(tools: &[&str], exclusive: bool) -> AppConfig {
        let mut config = AppConfig::default();
        config.agent_runtime = Some(AgentRuntimeOverride {
            enabled_tools: tools.iter().map(|name| name.to_string()).collect(),
            exclusive,
            ..Default::default()
        });
        config
    }

    /// 验证没有 Agent 覆盖时一切工具都放行。
    #[test]
    fn everything_is_allowed_without_an_agent_override() {
        assert!(whitelist_allows_tool(&AppConfig::default(), "write_memory"));
    }

    /// 验证非独占模式下空白名单不收窄。
    ///
    /// 翻转这条语义会让所有依赖旧行为的档案静默失去全部工具。
    #[test]
    fn an_empty_non_exclusive_whitelist_does_not_narrow() {
        let config = config_with_whitelist(&[], false);

        assert!(whitelist_allows_tool(&config, "write_memory"));
    }

    /// 验证独占模式下空白名单挡住一切。
    #[test]
    fn an_empty_exclusive_whitelist_blocks_everything() {
        let config = config_with_whitelist(&[], true);

        assert!(!whitelist_allows_tool(&config, "write_memory"));
    }

    /// 验证白名单之外的工具被判定为不可达。
    ///
    /// 这正是记忆契约需要的判断：工具注册了，却被档案挡在外面。
    #[test]
    fn a_tool_outside_the_whitelist_is_unreachable() {
        let config = config_with_whitelist(&["read_file", "run_command"], false);

        assert!(whitelist_allows_tool(&config, "read_file"));
        assert!(!whitelist_allows_tool(&config, "write_memory"));
    }

    /// 验证能点出白名单里已经不存在的工具名。
    #[test]
    fn stale_names_in_the_whitelist_are_reported() {
        let config = config_with_whitelist(&["read_file", "remember_fact"], false);
        let registered = vec!["read_file".to_string(), "write_memory".to_string()];

        assert_eq!(
            unknown_whitelist_tools(&config, &registered),
            vec!["remember_fact".to_string()]
        );
    }

    /// 验证没有覆盖时不报告任何死名字。
    #[test]
    fn no_stale_names_without_an_override() {
        let registered = vec!["read_file".to_string()];

        assert!(unknown_whitelist_tools(&AppConfig::default(), &registered).is_empty());
    }

    /// 验证已改名但替代已就位的旧名不算死名字。
    ///
    /// edit_file 早已换成 str_replace，迁移时当前名已补进白名单，功能
    /// 完好。报出来只会让人以为编辑能力坏了，转而去动一份没问题的配置。
    #[test]
    fn a_renamed_tool_with_its_replacement_present_is_not_reported() {
        let config = config_with_whitelist(&["edit_file", "str_replace"], false);
        let registered = vec!["str_replace".to_string()];

        assert!(unknown_whitelist_tools(&config, &registered).is_empty());
    }

    /// 验证替代本身也不存在时仍然报告。
    #[test]
    fn a_renamed_tool_is_reported_when_its_replacement_is_gone() {
        let config = config_with_whitelist(&["edit_file"], false);
        let registered = vec!["read_file".to_string()];

        assert_eq!(
            unknown_whitelist_tools(&config, &registered),
            vec!["edit_file".to_string()]
        );
    }
}
