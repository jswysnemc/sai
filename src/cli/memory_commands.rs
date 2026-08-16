use super::*;
use crate::memory::file_store::{Frontmatter, MemoryEntry, MemoryScope, MemoryType};

/// 执行 memory 子命令。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `args`: 子命令参数
///
/// 返回:
/// - 执行结果
pub(super) fn run_memory(paths: &SaiPaths, args: MemoryArgs) -> Result<()> {
    let config = AppConfig::load_or_default(paths)?;
    let store = MemoryStore::new(&config, paths);
    let workspace = crate::runtime_cwd::current_dir().ok();
    let library = store.notes(workspace.as_deref());
    match args.command {
        MemoryCommand::Stats => println!("{}", store.stats()?),
        MemoryCommand::Reset(args) => {
            if !confirm::confirm_destructive(t("clear assistant memory", "清空助手记忆"), args.yes)?
            {
                return Ok(());
            }
            println!("{}", clear_memory(paths)?);
        }
        MemoryCommand::List(args) => {
            let filter = join_message(args.filter).to_lowercase();
            for summary in library.list()? {
                // 空过滤词列出全部；有词时匹配标识或摘要
                if !filter.is_empty()
                    && !summary.name.to_lowercase().contains(&filter)
                    && !summary.description.to_lowercase().contains(&filter)
                {
                    continue;
                }
                println!(
                    "{}  [{}/{}]  {}",
                    summary.name,
                    summary.memory_type.as_str(),
                    scope_label(summary.scope),
                    summary.description
                );
            }
        }
        MemoryCommand::Show(args) => match library.load(&args.name)? {
            Some((entry, scope)) => {
                println!("# {}", entry.front.name);
                println!("{}", entry.front.description);
                println!(
                    "[{}/{}]",
                    entry.front.memory_type.as_str(),
                    scope_label(scope)
                );
                println!();
                println!("{}", entry.body);
            }
            None => bail!("{}: {}", t("memory not found", "未找到记忆"), args.name),
        },
        MemoryCommand::Remember(args) => {
            let content = join_message(args.content);
            // 空内容与功能关闭都写不进去，必须报错而不是假装成功
            if content.trim().is_empty() {
                bail!("{}", t("memory content is empty", "记忆内容为空"));
            }
            if !store.is_enabled() {
                bail!(
                    "{}",
                    t(
                        "memory is disabled; enable it in config first",
                        "记忆功能已关闭；请先在配置中启用"
                    )
                );
            }
            let memory_type = MemoryType::parse(&args.memory_type).ok_or_else(|| {
                anyhow::anyhow!(
                    "{}: {}",
                    t("unknown memory type", "未知记忆类型"),
                    args.memory_type
                )
            })?;
            let description = args
                .description
                .unwrap_or_else(|| first_line(&content).to_string());
            let scope = if args.global {
                MemoryScope::Global
            } else {
                MemoryScope::Project
            };
            let entry = MemoryEntry {
                front: Frontmatter {
                    name: args.name.clone(),
                    description: description.clone(),
                    memory_type,
                },
                body: content,
            };
            library.save(scope, &entry, &description)?;
            println!("{}: {}", t("remembered", "已记住"), args.name);
        }
        MemoryCommand::Forget(args) => {
            if library.delete(&args.name)? {
                println!("{}: {}", t("forgot", "已删除"), args.name);
            } else {
                bail!("{}: {}", t("memory not found", "未找到记忆"), args.name);
            }
        }
    }
    Ok(())
}

/// 清空助手记忆，并保留当前会话历史。
///
/// 参数:
/// - `paths`: Sai 路径
///
/// 返回:
/// - 面向用户的清理结果文本
pub(super) fn clear_memory(paths: &SaiPaths) -> Result<String> {
    let config = AppConfig::load_or_default(paths)?;
    MemoryStore::new(&config, paths).reset_all()?;
    Ok(t("cleared assistant memory", "已清空助手记忆").to_string())
}

/// 返回作用域的展示标识。
///
/// 参数:
/// - `scope`: 作用域
///
/// 返回:
/// - 小写标识
fn scope_label(scope: MemoryScope) -> &'static str {
    match scope {
        MemoryScope::Global => "global",
        MemoryScope::Project => "project",
    }
}

/// 取文本首行作为摘要兜底。
///
/// 参数:
/// - `text`: 正文
///
/// 返回:
/// - 首行文本
fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or(text).trim()
}
