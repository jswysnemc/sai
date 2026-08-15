pub mod evicted;
pub mod file_store;
mod store;

pub use evicted::EvictedTurn;
pub use store::MemoryStore;

/// 返回文件式记忆的根目录。
///
/// 参数:
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 记忆文件根目录
pub fn notes_dir(paths: &crate::paths::SaiPaths) -> std::path::PathBuf {
    paths.data_dir.join("memory").join("notes")
}
