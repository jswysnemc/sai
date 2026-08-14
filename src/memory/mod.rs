pub mod evicted;
pub mod file_store;
mod store;

pub use evicted::EvictedTurn;
pub use store::MemoryStore;

/// 返回文件式记忆的根目录。
///
/// 与逐出记录共用人格隔离规则：切换人格后两者都应该换成另一套，
/// 否则不同人格的偏好会互相覆盖。
///
/// 参数:
/// - `config`: 应用配置
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 记忆文件根目录
pub fn notes_dir(
    config: &crate::config::AppConfig,
    paths: &crate::paths::SaiPaths,
) -> std::path::PathBuf {
    config
        .active_persona_memory_data_dir(paths)
        .join("memory")
        .join("notes")
}
