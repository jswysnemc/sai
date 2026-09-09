use crate::config::AppConfig;
use crate::i18n::text as t;
use crate::memory::MemoryStore;
use crate::paths::SaiPaths;
use crate::state::StateStore;
use anyhow::Result;

/// 清空当前会话状态。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `all`: 是否同时清空全部记忆
///
/// 返回:
/// - 清空结果文本
pub fn clear_state(paths: &SaiPaths, all: bool) -> Result<String> {
    AppConfig::init_files(paths)?;
    let config = AppConfig::load_or_default(paths)?;
    StateStore::new(paths)?.reset_conversation()?;
    let memory = MemoryStore::new(&config, paths);
    if all {
        memory.reset_all()?;
    } else {
        memory.clear_evicted_context()?;
    }
    crate::plugins::clear_session_storage(&paths.state_dir, "direct-command")?;
    Ok(if all {
        t(
            "cleared current conversation history and all memory",
            "已清空当前会话历史与全部记忆",
        )
        .to_string()
    } else {
        t("cleared current conversation history", "已清空当前会话历史").to_string()
    })
}
