use super::{collect_top_level_items, SessionDataSummary};
use crate::paths::SaiPaths;
use crate::tools::todo::TodoStore;
use anyhow::Result;
use std::path::Path;

/// 在文件扫描结束后读取原生待办文件，统计当前活动条目。
///
/// 参数:
/// - `paths`: 应用目录
/// - `summaries`: 已有文件和数据库统计
///
/// 返回:
/// - 读取结果；单个会话的待办文件损坏时记到该会话错误字段
pub(super) async fn fill_counts(
    paths: &SaiPaths,
    summaries: &mut [SessionDataSummary],
) -> Result<()> {
    for summary in summaries.iter_mut() {
        let paths = paths.clone();
        let id = summary.id.clone();
        let workdir = Path::new(&summary.workspace_path).to_path_buf();
        let result = tokio::task::spawn_blocking(move || -> Result<_> {
            let (_, directory) =
                crate::state::state_dir_for_workspace_session(&paths, &workdir, &id)?;
            let todo_count = TodoStore::new(directory.join("todos.json"))
                .list()
                .map(|items| items.len());
            let items = collect_top_level_items(&directory)?;
            Ok((todo_count, items))
        })
        .await?;
        match result {
            Ok((todo_count, items)) => {
                summary.total_bytes = items.iter().map(|item| item.bytes).sum();
                summary.file_count = items.iter().map(|item| item.file_count).sum();
                summary.items = items;
                match todo_count {
                    Ok(count) => summary.todo_count = Some(count),
                    Err(error) => {
                        summary.todo_count = None;
                        summary.state_error = Some(match summary.state_error.take() {
                            Some(previous) => format!("{previous}; {error:#}"),
                            None => format!("{error:#}"),
                        });
                    }
                }
            }
            Err(error) => {
                summary.todo_count = None;
                summary.state_error = Some(match summary.state_error.take() {
                    Some(previous) => format!("{previous}; {error:#}"),
                    None => format!("{error:#}"),
                });
            }
        }
    }
    Ok(())
}
