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
