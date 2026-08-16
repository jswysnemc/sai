use super::StateStore;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const READ_FILES_FILE: &str = "read-files.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReadFileSnapshot {
    path: String,
    modified_ns: Option<u128>,
    length: u64,
}

impl StateStore {
    /// 记录本会话已经成功读取的文件及其当时的文件状态。
    ///
    /// 参数:
    /// - `path`: 已展开的文件路径
    ///
    /// 返回:
    /// - 写入结果
    pub(crate) fn record_read_file(&self, path: &Path) -> Result<()> {
        let path = normalized_path(path);
        let metadata = std::fs::metadata(&path)?;
        let mut entries = self.load_read_files();
        entries.retain(|entry| entry.path != path.display().to_string());
        entries.push(ReadFileSnapshot {
            path: path.display().to_string(),
            modified_ns: modified_ns(&metadata),
            length: metadata.len(),
        });
        std::fs::create_dir_all(&self.state_dir)?;
        std::fs::write(
            self.state_dir.join(READ_FILES_FILE),
            serde_json::to_vec(&entries)?,
        )?;
        Ok(())
    }

    /// 判断文件是否在本会话中读取过且之后没有被外部修改。
    ///
    /// 参数:
    /// - `path`: 已展开的文件路径
    ///
    /// 返回:
    /// - `Some("not_read")` 表示从未读取，`Some("changed")` 表示读取后发生变化，
    ///   `None` 表示可以继续编辑
    pub(crate) fn read_file_edit_block_reason(&self, path: &Path) -> Option<&'static str> {
        let path = normalized_path(path);
        let Some(entry) = self
            .load_read_files()
            .into_iter()
            .find(|entry| entry.path == path.display().to_string())
        else {
            return Some("not_read");
        };
        let Ok(metadata) = std::fs::metadata(path) else {
            return Some("changed");
        };
        if entry.length != metadata.len() || entry.modified_ns != modified_ns(&metadata) {
            Some("changed")
        } else {
            None
        }
    }

    fn load_read_files(&self) -> Vec<ReadFileSnapshot> {
        let file = self.state_dir.join(READ_FILES_FILE);
        std::fs::read(file)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }
}

fn normalized_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn modified_ns(metadata: &std::fs::Metadata) -> Option<u128> {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_and_invalidates_read_file_snapshot() {
        let temp = tempfile::tempdir().unwrap();
        let paths = crate::paths::SaiPaths::for_tests(temp.path());
        let store = StateStore::new(&paths).unwrap();
        let file = temp.path().join("sample.txt");
        std::fs::write(&file, "before").unwrap();

        assert_eq!(store.read_file_edit_block_reason(&file), Some("not_read"));
        store.record_read_file(&file).unwrap();
        assert_eq!(store.read_file_edit_block_reason(&file), None);
        std::fs::write(&file, "after").unwrap();
        assert_eq!(store.read_file_edit_block_reason(&file), Some("changed"));
    }
}
