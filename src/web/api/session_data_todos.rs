use super::{collect_top_level_items, SessionDataSummary};
use crate::{config::AppConfig, paths::SaiPaths, plugins::todo_view::TodoView};
use anyhow::Result;
use std::path::Path;

/// 【会话数据】【待办统计】在文件扫描结束后异步查询 Lua，避免在同步扫描中重建异步运行时
/// @param paths 应用目录；summaries 为已有文件和数据库统计
/// @returns 当前配置读取结果；单个会话查询失败记录到该会话错误字段
pub(super) async fn fill_counts(
    paths: &SaiPaths,
    summaries: &mut [SessionDataSummary],
) -> Result<()> {
    let config = AppConfig::load_or_default(paths)?;
    let view = TodoView::load(&config, paths).await?;
    for summary in summaries {
        let workdir = Path::new(&summary.workspace_path);
        let result = async {
            let (_, directory) =
                crate::state::state_dir_for_workspace_session(paths, workdir, &summary.id)?;
            let snapshot = view.snapshot(&summary.id, &directory, workdir).await?;
            // 1. 【会话数据】【归档后统计】快照可能接续旧记录，文件统计必须包含本次发布结果
            let items =
                tokio::task::spawn_blocking(move || collect_top_level_items(&directory)).await??;
            Ok::<_, anyhow::Error>((snapshot, items))
        }
        .await;
        match result {
            Ok((snapshot, items)) => {
                summary.todo_count = Some(snapshot.items.len());
                summary.total_bytes = items.iter().map(|item| item.bytes).sum();
                summary.file_count = items.iter().map(|item| item.file_count).sum();
                summary.items = items;
            }
            Err(error) => {
                summary.state_error = Some(match summary.state_error.take() {
                    Some(previous) => format!("{previous}; {error:#}"),
                    None => format!("{error:#}"),
                });
            }
        }
    }
    Ok(())
}
