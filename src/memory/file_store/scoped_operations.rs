use super::{
    memory_file, validate_name, FileMemoryLibrary, IndexDocument, MemoryDirectory, MemoryEntry,
    MemoryScope,
};
use anyhow::Result;

impl FileMemoryLibrary {
    /// 【记忆】【作用域定位】精确选择目录，不把缺失的项目目录回退到全局。
    ///
    /// 参数：`scope` 为目标作用域。
    /// 返回：目标目录；没有项目目录时返回 None。
    fn scoped_directory(&self, scope: MemoryScope) -> Option<&MemoryDirectory> {
        match scope {
            MemoryScope::Global => Some(&self.global),
            MemoryScope::Project => self.project.as_ref(),
        }
    }

    /// 【记忆】【作用域读取】读取指定作用域中的条目，不读取同名的其他条目。
    ///
    /// 参数：`name` 为条目标识，`scope` 为目标作用域。
    /// 返回：正文和元数据；条目或目录不存在时返回 None。
    pub fn load_scoped(&self, name: &str, scope: MemoryScope) -> Result<Option<MemoryEntry>> {
        let name = validate_name(name)?;
        let Some(directory) = self.scoped_directory(scope) else {
            return Ok(None);
        };
        memory_file::read(&directory.entry_path(name)?)
    }

    /// 【记忆】【索引读取】读取指定作用域中的索引提示。
    ///
    /// 参数：`name` 为条目标识，`scope` 为目标作用域。
    /// 返回：索引提示；不存在时返回 None。
    pub fn load_hook_scoped(&self, name: &str, scope: MemoryScope) -> Result<Option<String>> {
        let name = validate_name(name)?;
        let Some(directory) = self.scoped_directory(scope) else {
            return Ok(None);
        };
        let existing = std::fs::read_to_string(directory.index_path()).unwrap_or_default();
        let document = IndexDocument::parse(&existing);
        Ok(document
            .entries()
            .into_iter()
            .find(|entry| entry.file == format!("{name}.md"))
            .map(|entry| entry.hook.clone()))
    }

    /// 【记忆】【作用域删除】删除指定作用域的正文和索引，保留其他同名条目。
    ///
    /// 参数：`name` 为条目标识，`scope` 为目标作用域。
    /// 返回：本次是否删除了正文；目录不存在时返回 false。
    pub fn delete_scoped(&self, name: &str, scope: MemoryScope) -> Result<bool> {
        let name = validate_name(name)?;
        let Some(directory) = self.scoped_directory(scope) else {
            return Ok(false);
        };
        let removed = memory_file::remove(&directory.entry_path(name)?)?;
        if removed {
            self.update_index(directory, |document| {
                document.remove(&format!("{name}.md"));
            })?;
        }
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::file_store::{Frontmatter, MemoryType};

    /// 构造同名条目；参数为正文，返回测试条目。
    fn entry(body: &str) -> MemoryEntry {
        MemoryEntry {
            front: Frontmatter {
                name: "shared".into(),
                description: body.into(),
                memory_type: MemoryType::Reference,
            },
            body: body.into(),
        }
    }

    /// 验证读取和删除均限制在所选作用域，索引与正文保持一致。
    #[test]
    fn scoped_delete_preserves_shadowed_global_entry_and_hook() {
        let root = tempfile::tempdir().unwrap();
        let workspace = root.path().join("workspace");
        let library = FileMemoryLibrary::new(root.path(), Some(&workspace));
        library
            .save(MemoryScope::Global, &entry("global"), "global hook")
            .unwrap();
        library
            .save(MemoryScope::Project, &entry("project"), "project hook")
            .unwrap();
        assert_eq!(
            library
                .load_scoped("shared", MemoryScope::Global)
                .unwrap()
                .unwrap()
                .body,
            "global"
        );
        assert_eq!(
            library
                .load_hook_scoped("shared", MemoryScope::Project)
                .unwrap()
                .as_deref(),
            Some("project hook")
        );
        assert!(library
            .delete_scoped("shared", MemoryScope::Project)
            .unwrap());
        assert!(!library
            .delete_scoped("shared", MemoryScope::Project)
            .unwrap());
        assert!(library
            .load_scoped("shared", MemoryScope::Project)
            .unwrap()
            .is_none());
        assert!(library
            .load_hook_scoped("shared", MemoryScope::Project)
            .unwrap()
            .is_none());
        assert_eq!(
            library.load("shared").unwrap().unwrap().1,
            MemoryScope::Global
        );
        assert_eq!(
            library
                .load_hook_scoped("shared", MemoryScope::Global)
                .unwrap()
                .as_deref(),
            Some("global hook")
        );
    }

    /// 验证没有项目目录时不删除全局条目，并拒绝目录穿越标识。
    #[test]
    fn missing_project_never_falls_back_to_global() {
        let root = tempfile::tempdir().unwrap();
        let library = FileMemoryLibrary::new(root.path(), None);
        library
            .save(MemoryScope::Global, &entry("global"), "hook")
            .unwrap();
        assert!(!library
            .delete_scoped("shared", MemoryScope::Project)
            .unwrap());
        assert!(library
            .load_scoped("shared", MemoryScope::Project)
            .unwrap()
            .is_none());
        assert!(library
            .load_scoped("shared", MemoryScope::Global)
            .unwrap()
            .is_some());
        assert!(library
            .delete_scoped("../shared", MemoryScope::Global)
            .is_err());
    }
}
