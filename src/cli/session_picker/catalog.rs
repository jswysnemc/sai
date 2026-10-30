use super::model::Picker;
use crate::{paths::SaiPaths, state::ResumeTarget};
use anyhow::{Context, Result};
use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
};

/// 【会话选择】【目录加载】首屏只读当前索引，按需读取并缓存全部工作区。
pub(super) struct Catalog {
    paths: SaiPaths,
    directory: PathBuf,
    pending: Option<Receiver<Result<Vec<ResumeTarget>>>>,
    all_loaded: bool,
}

impl Catalog {
    /// 【会话选择】【当前目录】只读取当前工作区，为终端首屏建立列表。
    /// 参数: paths 为应用路径，directory 为当前目录，all 为初始范围；返回: 加载器与列表状态
    pub(super) fn new(paths: &SaiPaths, directory: &Path, all: bool) -> Result<(Self, Picker)> {
        let directory = crate::platform::windows_path::canonicalize(directory)?;
        let targets = crate::state::resume_catalog(paths, &directory, false)?;
        let workspace = crate::state::workspace_id_for_path(&directory);
        Ok((
            Self {
                paths: paths.clone(),
                directory,
                pending: None,
                all_loaded: false,
            },
            Picker::new(targets, workspace, all),
        ))
    }

    /// 【会话选择】【全量读取】在独立线程读取全部索引，不阻塞搜索、切换或取消。
    /// 参数: picker 为列表状态；返回: 后台任务启动结果，已有任务或缓存时不重复启动
    pub(super) fn load_all(&mut self, picker: &mut Picker) -> Result<()> {
        if self.all_loaded || self.pending.is_some() {
            return Ok(());
        }
        // 1. 【会话选择】【目录隔离】任务持有固定路径，取消或恢复后不读取新的进程目录
        let paths = self.paths.clone();
        let directory = self.directory.clone();
        let (sender, receiver) = mpsc::channel();
        thread::Builder::new()
            .name("sai-resume-catalog".into())
            .spawn(move || {
                let result = crate::state::resume_catalog(&paths, &directory, true);
                // 2. 【会话选择】【取消隔离】接收端随选择器释放，后台任务不修改终端或会话状态
                let _ = sender.send(result);
            })
            .context("Failed to start workspace session loader")?;
        self.pending = Some(receiver);
        picker.loading = true;
        picker.load_error = None;
        Ok(())
    }

    /// 【会话选择】【结果更新】非阻塞接收全量结果，错误保留当前列表供继续操作。
    /// 参数: picker 为列表状态；返回: 是否需要重新绘制
    pub(super) fn poll(&mut self, picker: &mut Picker) -> bool {
        let Some(receiver) = &self.pending else {
            return false;
        };
        let result = match receiver.try_recv() {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return false,
            Err(TryRecvError::Disconnected) => {
                Err(anyhow::anyhow!("Workspace session loader disconnected"))
            }
        };
        self.pending = None;
        picker.loading = false;
        match result {
            Ok(targets) => {
                self.all_loaded = true;
                picker.replace_targets(targets);
                picker.load_error = None;
            }
            Err(error) => picker.load_error = Some(format!("{error:#}")),
        }
        true
    }
}

#[cfg(test)]
#[path = "catalog_tests.rs"]
mod tests;
