mod delete;
mod evicted;
mod read;
mod support;
mod write;

use super::{ToolRegistry, ToolSpec};
use crate::config::AppConfig;
use crate::i18n::text as t;
use crate::paths::SaiPaths;

/// 注册全部记忆工具，含写入与删除。
///
/// 参数:
/// - `registry`: 工具注册表
/// - `config`: 应用配置
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 无
pub fn register(registry: &mut ToolRegistry, config: AppConfig, paths: SaiPaths) {
    if !config.memory_config().enabled {
        return;
    }
    register_readonly(registry, config.clone(), paths.clone());
    registry.register(
        ToolSpec::new(
            "write_memory",
            t(
                "Save one durable fact to memory as a file. Use for things worth carrying across sessions: who the user is, how they want you to work and why, ongoing project constraints, or pointers to external resources. One fact per entry. Writing an existing name updates it in place.",
                "把一条需要长期保留的事实写成记忆文件。适用于跨会话仍然成立的内容：用户是谁、要求你怎么工作及其理由、进行中的项目约束、外部资源指针。一条记忆只放一个事实。写入已存在的标识即为就地更新。",
            ),
            write::schema(),
            {
                let config = config.clone();
                let paths = paths.clone();
                move |args| {
                    let config = config.clone();
                    let paths = paths.clone();
                    async move { write::write_memory(args, config, paths).await }
                }
            },
        )
        .writes(),
    );
    registry.register(
        ToolSpec::new(
            "delete_memory",
            t(
                "Delete a memory that turned out to be wrong or no longer applies.",
                "删除一条已被证伪或不再适用的记忆。",
            ),
            delete::schema(),
            {
                let config = config.clone();
                let paths = paths.clone();
                move |args| {
                    let config = config.clone();
                    let paths = paths.clone();
                    async move { delete::delete_memory(args, config, paths).await }
                }
            },
        )
        .writes(),
    );
}

/// 注册只读的记忆工具。
///
/// 参数:
/// - `registry`: 工具注册表
/// - `config`: 应用配置
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 无
pub fn register_readonly(registry: &mut ToolRegistry, config: AppConfig, paths: SaiPaths) {
    if !config.memory_config().enabled {
        return;
    }
    registry.register(ToolSpec::new(
        "read_memory",
        t(
            "Read the full text of one memory by its identifier. The injected index lists only titles and hooks; read the entry itself before acting on it.",
            "按标识读取一条记忆的完整正文。注入的索引只有标题与提示，据此行动前先把正文读出来。",
        ),
        read::read_schema(),
        {
            let config = config.clone();
            let paths = paths.clone();
            move |args| {
                let config = config.clone();
                let paths = paths.clone();
                async move { read::read_memory(args, config, paths).await }
            }
        },
    ));
    registry.register(ToolSpec::new(
        "list_memory",
        t(
            "List every stored memory with its type and scope.",
            "列出全部已存记忆及其类型与作用域。",
        ),
        read::list_schema(),
        {
            let config = config.clone();
            let paths = paths.clone();
            move |args| {
                let config = config.clone();
                let paths = paths.clone();
                async move { read::list_memory(args, config, paths).await }
            }
        },
    ));
    registry.register(ToolSpec::new(
        "search_evicted_context",
        t(
            "Search conversation turns that were moved out of the active context window by compaction. Use this when the summary lacks a detail you need.",
            "检索被压缩清出上下文窗口的对话轮次。摘要里缺具体细节时用它回读原文。",
        ),
        evicted::schema(),
        {
            let config = config.clone();
            let paths = paths.clone();
            move |args| {
                let config = config.clone();
                let paths = paths.clone();
                async move { evicted::search_evicted_context(args, config, paths).await }
            }
        },
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::file_store::memory_contract;

    /// 构造启用记忆的配置。
    fn enabled_config() -> AppConfig {
        let mut config = AppConfig::default();
        config.plugins.memory.enabled = true;
        config
    }

    /// 验证记忆契约点名的工具确实注册了。
    ///
    /// 契约把工具名写死在提示词里，改名或漏注册都不会让任何东西报错，
    /// 只会让模型照着契约去调一个不存在的工具。
    #[test]
    fn every_tool_named_in_the_contract_is_registered() {
        let temp = std::env::temp_dir().join("sai-memory-tools-test");
        let paths = SaiPaths::for_tests(&temp);
        let mut registry = ToolRegistry::new();

        register(&mut registry, enabled_config(), paths);

        let contract = memory_contract();
        for name in ["write_memory", "read_memory", "delete_memory"] {
            assert!(contract.contains(name), "契约未提及 {name}");
            assert!(registry.contains(name), "契约提到 {name} 但它没有注册");
        }
    }

    /// 验证只读注册不包含写入与删除。
    ///
    /// 子智能体走的是这条路径，放进写工具等于让它们改主体的记忆。
    #[test]
    fn the_readonly_registration_withholds_mutating_tools() {
        let temp = std::env::temp_dir().join("sai-memory-tools-test");
        let paths = SaiPaths::for_tests(&temp);
        let mut registry = ToolRegistry::new();

        register_readonly(&mut registry, enabled_config(), paths);

        assert!(registry.contains("read_memory"));
        assert!(registry.contains("list_memory"));
        assert!(!registry.contains("write_memory"));
        assert!(!registry.contains("delete_memory"));
    }

    /// 验证关闭记忆后一个记忆工具都不注册。
    #[test]
    fn nothing_is_registered_when_memory_is_off() {
        let temp = std::env::temp_dir().join("sai-memory-tools-test");
        let paths = SaiPaths::for_tests(&temp);
        let mut config = AppConfig::default();
        config.plugins.memory.enabled = false;
        let mut registry = ToolRegistry::new();

        register(&mut registry, config, paths);

        assert!(!registry.contains("read_memory"));
        assert!(!registry.contains("write_memory"));
    }
}
