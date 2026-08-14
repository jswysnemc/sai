use anyhow::{bail, Result};
use std::path::{Path, PathBuf};

/// 索引文件名。
pub const INDEX_FILE_NAME: &str = "MEMORY.md";

/// 记忆文件扩展名。
const ENTRY_EXTENSION: &str = "md";

/// 标识允许的最大字符数。
const MAX_NAME_CHARS: usize = 80;

/// 一个记忆目录，对应一个作用域。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryDirectory {
    root: PathBuf,
}

impl MemoryDirectory {
    /// 返回跨项目通用的记忆目录。
    ///
    /// 参数:
    /// - `base`: 记忆根目录，已按人格隔离
    ///
    /// 返回:
    /// - 全局记忆目录
    pub fn global(base: &Path) -> Self {
        Self {
            root: base.join("global"),
        }
    }

    /// 返回某个工作区专属的记忆目录。
    ///
    /// 工作区路径编码进目录名而不是做成嵌套层级：嵌套会让 `/a` 与 `/a/b`
    /// 变成父子关系，删掉外层就连带删掉内层。
    ///
    /// 参数:
    /// - `base`: 记忆根目录，已按人格隔离
    /// - `workspace`: 工作区绝对路径
    ///
    /// 返回:
    /// - 项目记忆目录
    pub fn for_workspace(base: &Path, workspace: &Path) -> Self {
        Self {
            root: base.join("projects").join(encode_workspace(workspace)),
        }
    }

    /// 返回索引文件路径。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 索引路径
    pub fn index_path(&self) -> PathBuf {
        self.root.join(INDEX_FILE_NAME)
    }

    /// 返回某条记忆的文件路径。
    ///
    /// 参数:
    /// - `name`: 记忆标识
    ///
    /// 返回:
    /// - 文件路径；标识非法时报错
    pub fn entry_path(&self, name: &str) -> Result<PathBuf> {
        Ok(self.root.join(format!("{}.{ENTRY_EXTENSION}", validate_name(name)?)))
    }

    /// 创建目录。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 创建结果
    pub fn ensure(&self) -> Result<()> {
        std::fs::create_dir_all(&self.root)?;
        Ok(())
    }

    /// 列出目录下全部记忆的标识。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 按字典序排列的标识；目录不存在时为空
    pub fn list_names(&self) -> Result<Vec<String>> {
        if !self.root.is_dir() {
            return Ok(Vec::new());
        }
        let mut names = Vec::new();
        for entry in std::fs::read_dir(&self.root)? {
            let path = entry?.path();
            if path.extension().and_then(|value| value.to_str()) != Some(ENTRY_EXTENSION) {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
                continue;
            };
            // 索引本身也是 md，不能混进条目列表
            if path.file_name().and_then(|value| value.to_str()) == Some(INDEX_FILE_NAME) {
                continue;
            }
            names.push(stem.to_string());
        }
        names.sort();
        Ok(names)
    }
}

/// 校验记忆标识可安全用作文件名。
///
/// 标识来自模型输出，直接拼进路径等于把写文件的位置交给模型决定。
///
/// 参数:
/// - `name`: 待校验标识
///
/// 返回:
/// - 去掉首尾空白的标识；非法时报错
pub fn validate_name(name: &str) -> Result<&str> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        bail!("记忆标识不能为空");
    }
    if trimmed.chars().count() > MAX_NAME_CHARS {
        bail!("记忆标识过长，最多 {MAX_NAME_CHARS} 个字符");
    }
    // 路径分隔符与父目录记号会让写入落到目录之外
    if trimmed.contains(['/', '\\', ':']) || trimmed.contains("..") {
        bail!("记忆标识不能包含路径分隔符或父目录记号：{trimmed}");
    }
    if trimmed.starts_with('.') {
        bail!("记忆标识不能以点号开头：{trimmed}");
    }
    if trimmed.chars().any(|value| value.is_control()) {
        bail!("记忆标识不能包含控制字符");
    }
    Ok(trimmed)
}

/// 把工作区路径编码成单层目录名。
///
/// 参数:
/// - `workspace`: 工作区路径
///
/// 返回:
/// - 可用作目录名的编码结果
fn encode_workspace(workspace: &Path) -> String {
    let raw = workspace.to_string_lossy();
    let encoded: String = raw
        .chars()
        .map(|value| match value {
            '/' | '\\' | ':' => '-',
            value if value.is_control() => '-',
            value => value,
        })
        .collect();
    let trimmed = encoded.trim_end_matches('-');
    if trimmed.is_empty() {
        "root".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证工作区路径被编码成单层目录名。
    #[test]
    fn workspace_paths_collapse_into_one_segment() {
        let encoded = encode_workspace(Path::new("/home/snemc/workspace/sai"));

        assert_eq!(encoded, "-home-snemc-workspace-sai");
        assert!(!encoded.contains('/'));
    }

    /// 验证 Windows 盘符路径同样被编码成单层。
    #[test]
    fn windows_paths_collapse_into_one_segment() {
        let encoded = encode_workspace(Path::new(r"C:\Users\x\project"));

        assert!(!encoded.contains('\\'));
        assert!(!encoded.contains(':'));
    }

    /// 验证不同工作区不会编码成同一目录。
    ///
    /// 撞名会让两个项目的记忆互相污染。
    #[test]
    fn distinct_workspaces_do_not_collide() {
        let left = encode_workspace(Path::new("/home/a/project"));
        let right = encode_workspace(Path::new("/home/b/project"));

        assert_ne!(left, right);
    }

    /// 验证路径穿越标识被拒绝。
    ///
    /// 标识由模型给出，放行等于让它决定往哪写文件。
    #[test]
    fn traversal_names_are_rejected() {
        assert!(validate_name("../../etc/passwd").is_err());
        assert!(validate_name("a/b").is_err());
        assert!(validate_name(r"a\b").is_err());
        assert!(validate_name("..").is_err());
    }

    /// 验证隐藏文件标识被拒绝。
    #[test]
    fn dotfile_names_are_rejected() {
        assert!(validate_name(".gitignore").is_err());
    }

    /// 验证空标识被拒绝。
    #[test]
    fn blank_names_are_rejected() {
        assert!(validate_name("   ").is_err());
    }

    /// 验证常规标识通过校验。
    #[test]
    fn ordinary_names_pass() {
        assert_eq!(validate_name(" zh-writing ").unwrap(), "zh-writing");
        assert_eq!(validate_name("中文标识").unwrap(), "中文标识");
    }

    /// 验证条目路径落在目录之内。
    #[test]
    fn entry_paths_stay_inside_the_directory() {
        let directory = MemoryDirectory {
            root: PathBuf::from("/tmp/memory"),
        };

        let path = directory.entry_path("zh-writing").unwrap();

        assert_eq!(path, PathBuf::from("/tmp/memory/zh-writing.md"));
    }
}
