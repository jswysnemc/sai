use super::directory::{validate_name, MemoryDirectory};
use super::index_file::{IndexDocument, IndexEntry};
use super::memory_file::{self, MemoryEntry};
use super::memory_type::MemoryType;
use anyhow::Result;
use std::path::Path;

/// 记忆的作用域。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryScope {
    /// 跨项目通用
    Global,
    /// 仅在当前工作区有效
    Project,
}

/// 一条记忆在库中的位置与摘要。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemorySummary {
    /// 记忆标识
    pub name: String,
    /// 一句话摘要
    pub description: String,
    /// 条目类型
    pub memory_type: MemoryType,
    /// 所属作用域
    pub scope: MemoryScope,
}

/// 文件式记忆库。
///
/// 全局与项目两个目录并列查找，项目优先：同名条目下，项目里的那份
/// 覆盖全局那份，这样通用偏好可以在具体项目里被就地改写。
pub struct FileMemoryLibrary {
    global: MemoryDirectory,
    project: Option<MemoryDirectory>,
}

impl FileMemoryLibrary {
    /// 打开记忆库。
    ///
    /// 参数:
    /// - `base`: 记忆根目录，已按人格隔离
    /// - `workspace`: 当前工作区路径；无工作区时只有全局记忆
    ///
    /// 返回:
    /// - 记忆库
    pub fn new(base: &Path, workspace: Option<&Path>) -> Self {
        Self {
            global: MemoryDirectory::global(base),
            project: workspace.map(|path| MemoryDirectory::for_workspace(base, path)),
        }
    }

    /// 写入或更新一条记忆，并同步索引。
    ///
    /// 参数:
    /// - `scope`: 目标作用域
    /// - `entry`: 记忆内容
    /// - `hook`: 索引行里的一句话提示
    ///
    /// 返回:
    /// - 写入结果
    pub fn save(&self, scope: MemoryScope, entry: &MemoryEntry, hook: &str) -> Result<()> {
        let directory = self.directory(scope);
        directory.ensure()?;
        let name = validate_name(&entry.front.name)?;
        memory_file::write(&directory.entry_path(name)?, entry)?;
        // 索引与正文必须一起动：只写正文会让这条记忆在召回时不可见
        self.update_index(directory, |document| {
            document.upsert(IndexEntry {
                title: entry.front.description.clone(),
                file: format!("{name}.md"),
                hook: hook.trim().to_string(),
            });
        })
    }

    /// 读取一条记忆。
    ///
    /// 参数:
    /// - `name`: 记忆标识
    ///
    /// 返回:
    /// - 记忆内容与所属作用域；不存在时为 None
    pub fn load(&self, name: &str) -> Result<Option<(MemoryEntry, MemoryScope)>> {
        let name = validate_name(name)?;
        for (scope, directory) in self.lookup_order() {
            if let Some(entry) = memory_file::read(&directory.entry_path(name)?)? {
                return Ok(Some((entry, scope)));
            }
        }
        Ok(None)
    }

    /// 删除一条记忆，并同步索引。
    ///
    /// 参数:
    /// - `name`: 记忆标识
    ///
    /// 返回:
    /// - 是否确实删除了一条
    pub fn delete(&self, name: &str) -> Result<bool> {
        let name = validate_name(name)?;
        let mut deleted = false;
        for (scope, _) in self.lookup_order() {
            let directory = self.directory(scope);
            if memory_file::remove(&directory.entry_path(name)?)? {
                deleted = true;
                self.update_index(directory, |document| {
                    document.remove(&format!("{name}.md"));
                })?;
            }
        }
        Ok(deleted)
    }

    /// 列出全部记忆的摘要。
    ///
    /// 直接扫目录而不是读索引：索引可能被手改坏，目录才是事实来源。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 全局在后、项目在前的摘要列表
    pub fn list(&self) -> Result<Vec<MemorySummary>> {
        let mut found = Vec::new();
        for (scope, directory) in self.lookup_order() {
            for name in directory.list_names()? {
                let Some(entry) = memory_file::read(&directory.entry_path(&name)?)? else {
                    continue;
                };
                // 项目里的同名条目已经收过，全局那份不再重复
                if found
                    .iter()
                    .any(|existing: &MemorySummary| existing.name == entry.front.name)
                {
                    continue;
                }
                found.push(MemorySummary {
                    name: entry.front.name,
                    description: entry.front.description,
                    memory_type: entry.front.memory_type,
                    scope,
                });
            }
        }
        Ok(found)
    }

    /// 返回按查找优先级排列的目录。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 项目在前、全局在后的目录序列
    fn lookup_order(&self) -> Vec<(MemoryScope, &MemoryDirectory)> {
        let mut order = Vec::new();
        if let Some(project) = &self.project {
            order.push((MemoryScope::Project, project));
        }
        order.push((MemoryScope::Global, &self.global));
        order
    }

    /// 返回指定作用域的目录。
    ///
    /// 无工作区时项目作用域退回全局，避免记忆写到一个不存在的位置。
    ///
    /// 参数:
    /// - `scope`: 作用域
    ///
    /// 返回:
    /// - 目录
    fn directory(&self, scope: MemoryScope) -> &MemoryDirectory {
        match scope {
            MemoryScope::Project => self.project.as_ref().unwrap_or(&self.global),
            MemoryScope::Global => &self.global,
        }
    }

    /// 读改写一份索引。
    ///
    /// 参数:
    /// - `directory`: 目标目录
    /// - `mutate`: 索引变更操作
    ///
    /// 返回:
    /// - 写入结果
    fn update_index(
        &self,
        directory: &MemoryDirectory,
        mutate: impl FnOnce(&mut IndexDocument),
    ) -> Result<()> {
        let path = directory.index_path();
        let existing = std::fs::read_to_string(&path).unwrap_or_default();
        let mut document = IndexDocument::parse(&existing);
        mutate(&mut document);
        directory.ensure()?;
        std::fs::write(&path, document.render())?;
        Ok(())
    }

    /// 返回两个作用域的索引正文。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - （项目索引，全局索引），缺失时为空串
    pub fn index_contents(&self) -> (String, String) {
        let project = self
            .project
            .as_ref()
            .map(|directory| std::fs::read_to_string(directory.index_path()).unwrap_or_default())
            .unwrap_or_default();
        let global = std::fs::read_to_string(self.global.index_path()).unwrap_or_default();
        (project, global)
    }
}

#[cfg(test)]
mod tests {
    use super::super::frontmatter::Frontmatter;
    use super::*;

    /// 构造一个指向临时目录的记忆库。
    ///
    /// 参数:
    /// - `root`: 临时根目录
    ///
    /// 返回:
    /// - （记忆库，路径集合）
    fn library(root: &Path) -> FileMemoryLibrary {
        FileMemoryLibrary::new(&root.join("notes"), Some(&root.join("project")))
    }

    /// 构造一条记忆。
    ///
    /// 参数:
    /// - `name`: 标识
    /// - `body`: 正文
    ///
    /// 返回:
    /// - 记忆内容
    fn entry(name: &str, body: &str) -> MemoryEntry {
        MemoryEntry {
            front: Frontmatter {
                name: name.to_string(),
                description: format!("{name} 的摘要"),
                memory_type: MemoryType::Feedback,
            },
            body: body.to_string(),
        }
    }

    /// 验证写入后能读回。
    #[test]
    fn a_saved_memory_can_be_loaded() {
        let dir = tempfile::tempdir().unwrap();
        let library = library(dir.path());

        library
            .save(MemoryScope::Project, &entry("a", "正文"), "提示")
            .unwrap();

        let (found, scope) = library.load("a").unwrap().unwrap();
        assert_eq!(found.body, "正文");
        assert_eq!(scope, MemoryScope::Project);
    }

    /// 验证写入同时更新索引。
    ///
    /// 只写正文的话这条记忆在召回时等于不存在。
    #[test]
    fn saving_also_writes_the_index() {
        let dir = tempfile::tempdir().unwrap();
        let library = library(dir.path());

        library
            .save(MemoryScope::Project, &entry("a", "正文"), "提示")
            .unwrap();

        let (project_index, _) = library.index_contents();
        assert!(project_index.contains("a.md"));
        assert!(project_index.contains("提示"));
    }

    /// 验证项目记忆覆盖同名的全局记忆。
    #[test]
    fn a_project_memory_shadows_the_global_one() {
        let dir = tempfile::tempdir().unwrap();
        let library = library(dir.path());
        library
            .save(MemoryScope::Global, &entry("a", "全局"), "")
            .unwrap();
        library
            .save(MemoryScope::Project, &entry("a", "项目"), "")
            .unwrap();

        let (found, scope) = library.load("a").unwrap().unwrap();

        assert_eq!(found.body, "项目");
        assert_eq!(scope, MemoryScope::Project);
    }

    /// 验证列表不重复计入被覆盖的全局条目。
    #[test]
    fn listing_counts_a_shadowed_entry_once() {
        let dir = tempfile::tempdir().unwrap();
        let library = library(dir.path());
        library
            .save(MemoryScope::Global, &entry("a", "全局"), "")
            .unwrap();
        library
            .save(MemoryScope::Project, &entry("a", "项目"), "")
            .unwrap();

        let listed = library.list().unwrap();

        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].scope, MemoryScope::Project);
    }

    /// 验证删除同时清掉索引行。
    ///
    /// 索引里留着死链会让下一轮去读一个不存在的文件。
    #[test]
    fn deleting_also_clears_the_index_pointer() {
        let dir = tempfile::tempdir().unwrap();
        let library = library(dir.path());
        library
            .save(MemoryScope::Project, &entry("a", "正文"), "提示")
            .unwrap();

        assert!(library.delete("a").unwrap());

        let (project_index, _) = library.index_contents();
        assert!(!project_index.contains("a.md"));
        assert!(library.load("a").unwrap().is_none());
    }

    /// 验证删除不存在的记忆不报成功。
    #[test]
    fn deleting_a_missing_memory_reports_false() {
        let dir = tempfile::tempdir().unwrap();
        let library = library(dir.path());

        assert!(!library.delete("missing").unwrap());
    }

    /// 验证非法标识被拒绝而不是写到目录之外。
    #[test]
    fn an_unsafe_name_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let library = library(dir.path());

        assert!(library
            .save(MemoryScope::Project, &entry("../escape", "正文"), "")
            .is_err());
    }

    /// 验证没有工作区时项目作用域退回全局。
    ///
    /// 否则这条记忆会写到一个不存在的目录里，静默丢失。
    #[test]
    fn without_a_workspace_project_scope_falls_back_to_global() {
        let dir = tempfile::tempdir().unwrap();
        let library = FileMemoryLibrary::new(&dir.path().join("notes"), None);

        library
            .save(MemoryScope::Project, &entry("a", "正文"), "")
            .unwrap();

        assert!(library.load("a").unwrap().is_some());
    }
}
