use anyhow::{bail, Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// TODO 项状态。
#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TodoStatus {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

impl TodoStatus {
    /// 解析工具参数中的状态。
    ///
    /// 参数:
    /// - `value`: 状态文本
    ///
    /// 返回:
    /// - 解析后的状态
    pub(crate) fn parse(value: &str) -> Result<Self> {
        match value.trim() {
            "pending" => Ok(Self::Pending),
            "in_progress" => Ok(Self::InProgress),
            "completed" => Ok(Self::Completed),
            "cancelled" => Ok(Self::Cancelled),
            other => bail!("unsupported todo status: {other}"),
        }
    }

    /// 判断当前状态是否仍未完成。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 是否属于未完成状态
    pub(crate) fn is_unfinished(self) -> bool {
        matches!(self, Self::Pending | Self::InProgress)
    }
}

/// 会话级 TODO 项。
#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct TodoItem {
    pub(crate) id: String,
    pub(crate) text: String,
    pub(crate) status: TodoStatus,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
}

/// 归档后的一批已完成计划。
#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct TodoHistoryBatch {
    pub(crate) archived_at: String,
    pub(crate) items: Vec<TodoItem>,
}

/// 会话级 TODO 持久化存储。
#[derive(Debug, Clone)]
pub(crate) struct TodoStore {
    file: PathBuf,
}

impl TodoStore {
    /// 创建绑定指定状态文件的 TODO 存储。
    ///
    /// 参数:
    /// - `file`: 当前会话 TODO 文件
    ///
    /// 返回:
    /// - TODO 存储
    pub(crate) fn new(file: PathBuf) -> Self {
        Self { file }
    }

    /// 读取当前会话全部 TODO 项。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - TODO 项列表
    pub(crate) fn list(&self) -> Result<Vec<TodoItem>> {
        if !self.file.exists() {
            return Ok(Vec::new());
        }
        let content = std::fs::read_to_string(&self.file)
            .with_context(|| format!("failed to read todo file {}", self.file.display()))?;
        if content.trim().is_empty() {
            return Ok(Vec::new());
        }
        serde_json::from_str(&content)
            .with_context(|| format!("failed to parse todo file {}", self.file.display()))
    }

    /// 新增一个 TODO 项。
    ///
    /// 参数:
    /// - `text`: TODO 内容
    ///
    /// 返回:
    /// - 新增后的 TODO 项
    #[cfg(test)]
    pub(crate) fn add(&self, text: &str) -> Result<TodoItem> {
        Ok(self.add_many(&[text.to_string()], None)?.remove(0))
    }

    /// 批量新增 TODO 项,可指定 1 起始的插入位置。
    ///
    /// 参数:
    /// - `texts`: TODO 内容列表
    /// - `index`: 可选插入位置(1 起始,新条目插在该序号之前),缺省或超界时追加到末尾
    ///
    /// 返回:
    /// - 本次新增的 TODO 项列表
    pub(crate) fn add_many(&self, texts: &[String], index: Option<usize>) -> Result<Vec<TodoItem>> {
        if texts.is_empty() {
            bail!("todo add requires text or texts")
        }
        let mut items = self.list()?;
        let now = Utc::now().to_rfc3339();
        // 1. 逐条构造新条目,序号后缀保证同毫秒批量创建时 id 仍唯一
        let created = texts
            .iter()
            .enumerate()
            .map(|(offset, text)| {
                Ok(TodoItem {
                    id: format!(
                        "todo_{}_{offset}_{}",
                        Utc::now().timestamp_millis(),
                        rand::random::<u16>()
                    ),
                    text: required_text(text)?.to_string(),
                    status: TodoStatus::Pending,
                    created_at: now.clone(),
                    updated_at: now.clone(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        // 2. 按插入位置写入,1 起始序号转 0 起始下标并夹取到列表长度内
        let position = index
            .map(|value| value.saturating_sub(1).min(items.len()))
            .unwrap_or(items.len());
        items.splice(position..position, created.iter().cloned());
        self.save(&items)?;
        Ok(created)
    }

    /// 把 id 或 1 起始序号解析为列表下标。
    ///
    /// 参数:
    /// - `id`: 可选条目 id
    /// - `index`: 可选 1 起始序号,id 缺省时使用
    ///
    /// 返回:
    /// - 目标条目的 0 起始下标
    pub(crate) fn locate(&self, id: Option<&str>, index: Option<usize>) -> Result<usize> {
        let items = self.list()?;
        if let Some(id) = id.map(str::trim).filter(|value| !value.is_empty()) {
            return items
                .iter()
                .position(|item| item.id == id)
                .with_context(|| format!("todo item not found: {id}"));
        }
        let Some(index) = index else {
            bail!("todo update/remove requires id or index")
        };
        if index == 0 || index > items.len() {
            bail!(
                "todo index out of range: {index} (list has {} items)",
                items.len()
            )
        }
        Ok(index - 1)
    }

    /// 更新指定下标的 TODO 项。
    ///
    /// 参数:
    /// - `position`: 目标条目的 0 起始下标(由 locate 解析)
    /// - `text`: 可选的新内容
    /// - `status`: 可选的新状态
    ///
    /// 返回:
    /// - 更新后的 TODO 项
    pub(crate) fn update_at(
        &self,
        position: usize,
        text: Option<&str>,
        status: Option<TodoStatus>,
    ) -> Result<TodoItem> {
        if text.is_none() && status.is_none() {
            bail!("todo update requires text or status")
        }
        let mut items = self.list()?;
        if position >= items.len() {
            bail!("todo item not found at position {position}")
        }
        if let Some(next_status) = status {
            validate_transition(&items, position, next_status)?;
        }
        let item = &mut items[position];
        if let Some(text) = text {
            item.text = required_text(text)?.to_string();
        }
        if let Some(status) = status {
            item.status = status;
        }
        item.updated_at = Utc::now().to_rfc3339();
        let updated = item.clone();
        self.save(&items)?;
        Ok(updated)
    }

    /// 删除指定下标的 TODO 项。
    ///
    /// 参数:
    /// - `position`: 目标条目的 0 起始下标(由 locate 解析)
    ///
    /// 返回:
    /// - 被删除的 TODO 项
    pub(crate) fn remove_at(&self, position: usize) -> Result<TodoItem> {
        let mut items = self.list()?;
        if position >= items.len() {
            bail!("todo item not found at position {position}")
        }
        let removed = items.remove(position);
        self.save(&items)?;
        Ok(removed)
    }

    /// 判断当前会话是否存在未完成项。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 是否存在未完成项
    pub(crate) fn has_unfinished(&self) -> Result<bool> {
        Ok(self.list()?.iter().any(|item| item.status.is_unfinished()))
    }

    /// 保存当前会话全部 TODO 项；计划全部完成后归档并清空活动列表。
    ///
    /// 参数:
    /// - `items`: TODO 项列表
    ///
    /// 返回:
    /// - 保存是否成功
    fn save(&self, items: &[TodoItem]) -> Result<()> {
        if items.is_empty() {
            return self.write_items(&[]);
        }
        // 全部完成后进入历史，活动列表重置，避免无限增长。
        if items.iter().all(|item| !item.status.is_unfinished()) {
            self.append_history(items)?;
            return self.write_items(&[]);
        }
        self.write_items(items)
    }

    /// 将已完成计划追加到历史文件。
    ///
    /// 参数:
    /// - `items`: 要归档的 TODO 项
    ///
    /// 返回:
    /// - 写入是否成功
    fn append_history(&self, items: &[TodoItem]) -> Result<()> {
        let history_file = self.history_file();
        if let Some(parent) = history_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut history = if history_file.exists() {
            let content = std::fs::read_to_string(&history_file).with_context(|| {
                format!("failed to read todo history {}", history_file.display())
            })?;
            if content.trim().is_empty() {
                Vec::new()
            } else {
                serde_json::from_str::<Vec<TodoHistoryBatch>>(&content).with_context(|| {
                    format!("failed to parse todo history {}", history_file.display())
                })?
            }
        } else {
            Vec::new()
        };
        history.push(TodoHistoryBatch {
            archived_at: Utc::now().to_rfc3339(),
            items: items.to_vec(),
        });
        let content = serde_json::to_string_pretty(&history)?;
        std::fs::write(&history_file, format!("{content}\n"))
            .with_context(|| format!("failed to write todo history {}", history_file.display()))
    }

    /// 将活动 TODO 列表写入主文件。
    ///
    /// 参数:
    /// - `items`: TODO 项列表
    ///
    /// 返回:
    /// - 写入是否成功
    fn write_items(&self, items: &[TodoItem]) -> Result<()> {
        if let Some(parent) = self.file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(items)?;
        std::fs::write(&self.file, format!("{content}\n"))
            .with_context(|| format!("failed to write todo file {}", self.file.display()))
    }

    /// 返回历史归档文件路径。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 与 todos.json 同目录下的 todos.history.json
    fn history_file(&self) -> PathBuf {
        self.file
            .parent()
            .map(|parent| parent.join("todos.history.json"))
            .unwrap_or_else(|| PathBuf::from("todos.history.json"))
    }

    /// 返回当前存储文件路径。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - TODO 文件路径
    #[cfg(test)]
    pub(crate) fn file(&self) -> &std::path::Path {
        &self.file
    }
}

/// 校验 TODO 必须按照列表顺序逐项推进。
fn validate_transition(items: &[TodoItem], index: usize, next: TodoStatus) -> Result<()> {
    if matches!(next, TodoStatus::InProgress | TodoStatus::Completed)
        && items[..index]
            .iter()
            .any(|item| item.status.is_unfinished())
    {
        bail!("complete earlier todo items before advancing this item")
    }
    if next == TodoStatus::InProgress
        && items.iter().enumerate().any(|(other_index, item)| {
            other_index != index && item.status == TodoStatus::InProgress
        })
    {
        bail!("only one todo item can be in progress")
    }
    Ok(())
}

/// 校验并返回非空 TODO 内容。
///
/// 参数:
/// - `text`: 待校验内容
///
/// 返回:
/// - 去除首尾空白后的内容
fn required_text(text: &str) -> Result<&str> {
    let text = text.trim();
    if text.is_empty() {
        bail!("todo text is required")
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证 TODO 项能够持久化并更新。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn persists_and_updates_items() {
        let dir = tempfile::tempdir().unwrap();
        let store = TodoStore::new(dir.path().join("todos.json"));

        let item = store.add("实现持久化").unwrap();
        assert!(store.file().exists());
        assert!(store.has_unfinished().unwrap());

        let position = store.locate(Some(&item.id), None).unwrap();
        store
            .update_at(position, None, Some(TodoStatus::InProgress))
            .unwrap();
        let updated = store
            .update_at(position, None, Some(TodoStatus::Completed))
            .unwrap();
        assert_eq!(updated.status, TodoStatus::Completed);
        assert!(!store.has_unfinished().unwrap());
        // 计划全部完成后自动归档，活动列表清空。
        assert!(store.list().unwrap().is_empty());
        let history_file = store.file().parent().unwrap().join("todos.history.json");
        assert!(history_file.exists());
        let history: Vec<TodoHistoryBatch> =
            serde_json::from_str(&std::fs::read_to_string(history_file).unwrap()).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].items.len(), 1);
        assert_eq!(history[0].items[0].status, TodoStatus::Completed);

        let reopened = TodoStore::new(store.file().to_path_buf());
        assert!(reopened.list().unwrap().is_empty());
    }

    /// 验证新计划在归档后从空白列表开始。
    #[test]
    fn starts_fresh_plan_after_archive() {
        let dir = tempfile::tempdir().unwrap();
        let store = TodoStore::new(dir.path().join("todos.json"));
        let first = store.add("done plan").unwrap();
        let position = store.locate(Some(&first.id), None).unwrap();
        store
            .update_at(position, None, Some(TodoStatus::Completed))
            .unwrap();
        assert!(store.list().unwrap().is_empty());

        let next = store.add("new plan").unwrap();
        assert_eq!(store.list().unwrap().len(), 1);
        assert_eq!(store.list().unwrap()[0].id, next.id);
        assert_eq!(store.list().unwrap()[0].text, "new plan");
    }

    /// 验证不同会话文件相互隔离。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn isolates_items_by_session_file() {
        let dir = tempfile::tempdir().unwrap();
        let first = TodoStore::new(dir.path().join("first/todos.json"));
        let second = TodoStore::new(dir.path().join("second/todos.json"));

        first.add("first session").unwrap();

        assert_eq!(first.list().unwrap().len(), 1);
        assert!(second.list().unwrap().is_empty());
    }

    #[test]
    fn enforces_sequential_single_in_progress_lifecycle() {
        let dir = tempfile::tempdir().unwrap();
        let store = TodoStore::new(dir.path().join("todos.json"));
        store
            .add_many(&["first".to_string(), "second".to_string()], None)
            .unwrap();

        assert!(store
            .update_at(1, None, Some(TodoStatus::InProgress))
            .is_err());
        store
            .update_at(0, None, Some(TodoStatus::InProgress))
            .unwrap();
        assert!(store
            .update_at(1, None, Some(TodoStatus::InProgress))
            .is_err());
        store
            .update_at(0, None, Some(TodoStatus::Completed))
            .unwrap();
        store
            .update_at(1, None, Some(TodoStatus::InProgress))
            .unwrap();
    }

    #[test]
    fn batch_add_inserts_at_one_based_index() {
        let dir = tempfile::tempdir().unwrap();
        let store = TodoStore::new(dir.path().join("todos.json"));
        store
            .add_many(&["a".to_string(), "d".to_string()], None)
            .unwrap();

        let created = store
            .add_many(&["b".to_string(), "c".to_string()], Some(2))
            .unwrap();

        assert_eq!(created.len(), 2);
        let texts = store
            .list()
            .unwrap()
            .into_iter()
            .map(|item| item.text)
            .collect::<Vec<_>>();
        assert_eq!(texts, vec!["a", "b", "c", "d"]);
    }

    #[test]
    fn locate_resolves_id_and_one_based_index() {
        let dir = tempfile::tempdir().unwrap();
        let store = TodoStore::new(dir.path().join("todos.json"));
        let created = store
            .add_many(&["a".to_string(), "b".to_string()], None)
            .unwrap();

        assert_eq!(store.locate(Some(&created[1].id), None).unwrap(), 1);
        assert_eq!(store.locate(None, Some(1)).unwrap(), 0);
        assert!(store.locate(None, Some(0)).is_err());
        assert!(store.locate(None, Some(3)).is_err());
        assert!(store.locate(None, None).is_err());
    }

    #[test]
    fn allows_completing_pending_item_when_earlier_items_finished() {
        let dir = tempfile::tempdir().unwrap();
        let store = TodoStore::new(dir.path().join("todos.json"));
        store
            .add_many(&["first".to_string(), "second".to_string()], None)
            .unwrap();

        // 前面存在未完成条目时仍拒绝跳序完成
        assert!(store
            .update_at(1, None, Some(TodoStatus::Completed))
            .is_err());
        store
            .update_at(0, None, Some(TodoStatus::Completed))
            .unwrap();
        let second = store
            .update_at(1, None, Some(TodoStatus::Completed))
            .unwrap();
        assert_eq!(second.status, TodoStatus::Completed);
    }
}
