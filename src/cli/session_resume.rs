use crate::{
    paths::SaiPaths,
    state::{ResumeTarget, StateStore},
};
use anyhow::{Context, Result};
use std::path::PathBuf;

/// 【会话恢复】【目录事务】准备失败时恢复进程原目录，成功后保持目标目录。
pub(super) struct DirectoryChange {
    original: PathBuf,
    committed: bool,
}
impl DirectoryChange {
    /// 【会话恢复】【切换目录】校验目标后切换进程目录，不改变会话指针。
    /// 参数: paths 为应用路径，target 为完整目标；返回: 回滚守卫
    pub fn enter(paths: &SaiPaths, target: &ResumeTarget) -> Result<Self> {
        let directory = target.directory(paths)?;
        let original = std::env::current_dir()?;
        std::env::set_current_dir(&directory)
            .with_context(|| format!("Cannot enter workspace: {}", directory.display()))?;
        Ok(Self {
            original,
            committed: false,
        })
    }
    /// 【会话恢复】【提交目录】确认目录切换成功。
    /// 参数: 无；返回: 无
    pub fn commit(mut self) {
        self.committed = true;
    }
}
impl Drop for DirectoryChange {
    /// 【会话恢复】【失败回滚】在准备失败时恢复原进程目录。
    /// 参数: 无；返回: 无
    fn drop(&mut self) {
        if !self.committed {
            let _ = std::env::set_current_dir(&self.original);
        }
    }
}

/// 【会话恢复】【CLI 执行】打开精确目标后更新所在工作区的活动指针。
/// 参数: paths 为应用路径，target 为选择目标；返回: 恢复结果文本
pub(super) fn activate(paths: &SaiPaths, target: &ResumeTarget) -> Result<String> {
    let directory = target.directory(paths)?;
    let change = DirectoryChange::enter(paths, target)?;
    let state = StateStore::for_workspace_session(paths, &directory, &target.session.info.id)?;
    state.init_files()?;
    crate::state::switch_workspace_session(paths, &directory, state.session_id())?;
    change.commit();
    Ok(message(target))
}

/// 【会话恢复】【结果展示】显示标题、标识与目录，便于核对跨区恢复目标。
/// 参数: target 为完整目标；返回: 确认文本
pub(super) fn message(target: &ResumeTarget) -> String {
    format!(
        "{}: {}  {}\n{}",
        crate::i18n::text("Resumed session", "已恢复会话"),
        target.session.info.id,
        target.session.info.title,
        target
            .workspace_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_default()
    )
}
