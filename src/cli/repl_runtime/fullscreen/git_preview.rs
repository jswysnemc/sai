//! 单文件差异的异步加载、比较模式与浏览状态。

use super::git_preview_patch::{parse, PreviewContent};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum DiffMode {
    #[default]
    Working,
    Staged,
    Unstaged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReviewFocus {
    Files,
    Diff,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReviewHover {
    Close,
    Mode(DiffMode),
    File(usize),
}

impl DiffMode {
    /// 【Git预览】【比较参数】转换为既有只读接口的比较模式。
    /// 参数: 无；返回: 接口模式名称。
    pub(super) fn api(self) -> &'static str {
        match self {
            Self::Working => "working_tree",
            Self::Staged => "staged",
            Self::Unstaged => "unstaged",
        }
    }

    /// 【Git预览】【模式名称】返回比较控件上的名称。
    /// 参数: 无；返回: 界面名称。
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Working => "Working tree",
            Self::Staged => "Staged",
            Self::Unstaged => "Unstaged",
        }
    }

    /// 【Git预览】【模式切换】循环选择三种只读比较，不触发 Git 写入。
    /// 参数: 无；返回: 下一个模式。
    pub(super) fn next(self) -> Self {
        match self {
            Self::Working => Self::Staged,
            Self::Staged => Self::Unstaged,
            Self::Unstaged => Self::Working,
        }
    }
}

#[derive(Default)]
pub(super) struct GitPreview {
    pub(super) path: Option<String>,
    pub(super) mode: DiffMode,
    pub(super) content: Option<PreviewContent>,
    pub(super) error: Option<String>,
    pub(super) scroll: usize,
    pub(super) horizontal: usize,
    pub(super) file_scroll: usize,
    pub(super) focus: Option<ReviewFocus>,
    pub(super) hovered: Option<ReviewHover>,
    directory: PathBuf,
    receiver: Option<Receiver<Result<PreviewContent, String>>>,
    task: Option<tokio::task::JoinHandle<()>>,
}

impl GitPreview {
    /// 【Git预览】【打开文件】记录文件和工作区，取消旧请求后异步读取新差异。
    /// 参数: directory 为工作目录，path 为仓库相对路径；返回: 无。
    pub(super) fn open(&mut self, directory: PathBuf, path: String) {
        let file_scroll = self.file_scroll;
        self.close();
        self.directory = directory;
        self.path = Some(path);
        self.mode = DiffMode::Working;
        self.file_scroll = file_scroll;
        self.focus = Some(ReviewFocus::Diff);
        self.load();
    }

    /// 【Git预览】【关闭窗口】取消后台任务并清除可见内容，晚到结果不会进入下一次预览。
    /// 参数: 无；返回: 无。
    pub(super) fn close(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
        self.receiver = None;
        self.path = None;
        self.content = None;
        self.error = None;
        self.scroll = 0;
        self.horizontal = 0;
        self.file_scroll = 0;
        self.focus = None;
        self.hovered = None;
    }

    /// 【Git预览】【切换比较】仅在模式发生变化时重新读取，保持所选文件不变。
    /// 参数: mode 为比较模式；返回: 无。
    pub(super) fn set_mode(&mut self, mode: DiffMode) {
        if self.mode != mode {
            self.mode = mode;
            self.load();
        }
    }

    /// 【Git预览】【请求差异】复用经过路径校验的只读接口，超时和失败转为界面状态。
    /// 参数: 无；返回: 无。
    fn load(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
        self.receiver = None;
        self.content = None;
        self.error = None;
        self.scroll = 0;
        self.horizontal = 0;
        let Some(path) = self.path.clone() else {
            return;
        };
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            self.error = Some("Diff loading is unavailable".into());
            return;
        };
        let directory = self.directory.clone();
        let mode = self.mode;
        let (sender, receiver) = mpsc::channel();
        self.receiver = Some(receiver);
        self.task = Some(handle.spawn(async move {
            let result = tokio::time::timeout(
                Duration::from_secs(5),
                crate::web::git_diff(&directory, mode.api(), Some(&path)),
            )
            .await;
            let result = match result {
                Ok(Ok(diff)) => tokio::task::spawn_blocking(move || {
                    let mut content = parse(&diff.patch, diff.truncated);
                    super::git_preview_code::cache(&mut content, &path);
                    content
                })
                .await
                .map_err(|_| "Diff rendering failed".to_string()),
                Ok(Err(error)) => Err(error.to_string()),
                Err(_) => Err("Diff loading timed out".into()),
            };
            let _ = sender.send(result);
        }));
    }

    /// 【Git预览】【异步取回】接收当前请求结果，工作区变化时关闭旧预览。
    /// 参数: directory 为当前工作目录；返回: 界面是否需要重绘。
    pub(super) fn tick(&mut self, directory: &Path) -> bool {
        if self.path.is_none() {
            return false;
        }
        if self.directory != directory {
            self.close();
            return true;
        }
        let Some(result) = self
            .receiver
            .as_ref()
            .and_then(|receiver| receiver.try_recv().ok())
        else {
            return false;
        };
        self.receiver = None;
        self.task = None;
        match result {
            Ok(content) => self.content = Some(content),
            Err(error) => self.error = Some(error),
        }
        true
    }

    /// 【Git预览】【加载唤醒】加载期间短周期检查结果，完成后停止预览专用轮询。
    /// 参数: 无；返回: 加载等待时间，非加载状态为空。
    pub(super) fn wait(&self) -> Option<Duration> {
        self.receiver.as_ref().map(|_| Duration::from_millis(50))
    }

    /// 【Git预览】【内容滚动】按可见行数限制纵向浏览，始终保留窗口标题和操作栏。
    /// 参数: delta 为滚动量，visible 为可见内容行数；返回: 滚动是否变化。
    pub(super) fn scroll_by(&mut self, delta: isize, visible: usize) -> bool {
        let before = self.scroll;
        let max = self
            .content
            .as_ref()
            .map_or(0, |content| content.lines.len().saturating_sub(visible));
        self.scroll = self.scroll.saturating_add_signed(delta).min(max);
        self.scroll != before
    }

    /// 【终端审查】【文件列表滚动】只改变文件清单偏移，不影响差异正文或聊天。
    /// 参数: delta 为滚动量，visible 为可见文件数，count 为文件总数；返回: 是否变化。
    pub(super) fn scroll_files(&mut self, delta: isize, visible: usize, count: usize) -> bool {
        let before = self.file_scroll;
        self.file_scroll = self
            .file_scroll
            .saturating_add_signed(delta)
            .min(count.saturating_sub(visible));
        self.file_scroll != before
    }
}

impl Drop for GitPreview {
    /// 【Git预览】【任务收尾】退出全屏时取消未完成读取。
    /// 参数: 无；返回: 无。
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}
