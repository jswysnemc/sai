use super::*;

pub(super) fn run_memory(paths: &SaiPaths, args: MemoryArgs) -> Result<()> {
    let config = AppConfig::load_or_default(paths)?;
    let store = MemoryStore::new(&config, paths);
    match args.command {
        MemoryCommand::Stats => println!("{}", store.stats()?),
        MemoryCommand::Reset(args) => {
            println!("{}", clear_memory(paths, args.include_skills)?);
        }
        MemoryCommand::Search(args) => {
            let query = join_message(args.query);
            let limit = args.limit.unwrap_or(10);
            println!("{}", store.recall_memories(&query, limit, args.forgotten)?);
        }
        MemoryCommand::Remember(args) => {
            let content = join_message(args.content);
            let id = store.remember_fact(&content, &args.source)?;
            println!("{}: {id}", t("remembered fact", "已记住事实"));
        }
    }
    Ok(())
}

/// 清空助手记忆，并保留当前会话历史。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `include_skills`: 是否同时清理自动学习的技能
///
/// 返回:
/// - 面向用户的清理结果文本
pub(super) fn clear_memory(paths: &SaiPaths, include_skills: bool) -> Result<String> {
    let config = AppConfig::load_or_default(paths)?;
    let store = MemoryStore::new(&config, paths);
    store.reset_all(include_skills)?;
    Ok(t("cleared assistant memory", "已清空助手记忆").to_string())
}
