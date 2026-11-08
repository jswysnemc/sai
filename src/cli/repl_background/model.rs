//! 任务筛选、稳定选择和日志分页状态，不依赖终端输入输出。

use super::service::{Request, Update};
use serde::Deserialize;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub(in crate::cli) struct Task {
    pub id: String,
    pub label: String,
    pub command: String,
    pub cwd: String,
    pub pid: u32,
    pub status: String,
    pub started_at: u64,
    pub updated_at: u64,
    pub timeout_seconds: u64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Filter {
    #[default]
    All,
    Running,
    Finished,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Stream {
    #[default]
    Stdout,
    Stderr,
    All,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub(super) struct Output {
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    #[serde(default)]
    pub stdout_truncated: bool,
    #[serde(default)]
    pub stderr_truncated: bool,
}

pub(super) struct TaskManager {
    pub tasks: Vec<Task>,
    pub selected_id: Option<String>,
    pub hovered: Option<String>,
    pub filter: Filter,
    pub pager: bool,
    pub stream: Stream,
    pub offset: usize,
    pub follow: bool,
    pub output: Output,
    pub output_id: Option<String>,
    pub output_error: Option<String>,
    pub confirm: Option<Request>,
    pub busy: bool,
    pub loading: bool,
    pub notice: Option<(String, bool, Instant)>,
}

impl Default for TaskManager {
    /// 【后台任务】【初始状态】创建等待首次查询的任务管理状态。
    /// 参数: 无；返回: 默认列表和日志状态。
    fn default() -> Self {
        Self {
            tasks: Vec::new(),
            selected_id: None,
            hovered: None,
            filter: Filter::All,
            pager: false,
            stream: Stream::Stdout,
            offset: 0,
            follow: true,
            output: Output::default(),
            output_id: None,
            output_error: None,
            confirm: None,
            busy: false,
            loading: true,
            notice: None,
        }
    }
}

impl TaskManager {
    /// 【后台任务】【范围筛选】获取当前筛选范围内的条目下标。
    /// 参数: 无；返回: 原始任务列表中的下标。
    pub fn visible(&self) -> Vec<usize> {
        self.tasks
            .iter()
            .enumerate()
            .filter(|(_, task)| match self.filter {
                Filter::All => true,
                Filter::Running => task.status == "running",
                Filter::Finished => task.status != "running",
            })
            .map(|(index, _)| index)
            .collect()
    }

    /// 【后台任务】【当前选择】按稳定标识获取任务，避免刷新改变操作目标。
    /// 参数: 无；返回: 当前选中任务。
    pub fn selected(&self) -> Option<&Task> {
        self.tasks
            .iter()
            .find(|task| Some(&task.id) == self.selected_id.as_ref())
    }

    /// 【后台任务】【选择修复】保留有效选择，筛选或删除后选择首个可见条目。
    /// 参数: 无；返回: 无。
    pub fn reconcile(&mut self) {
        let visible = self.visible();
        if !visible
            .iter()
            .any(|index| Some(&self.tasks[*index].id) == self.selected_id.as_ref())
        {
            self.selected_id = visible.first().map(|index| self.tasks[*index].id.clone());
            self.offset = 0;
            self.follow = true;
        }
        if self.selected_id.is_none() {
            self.pager = false;
        }
    }

    /// 【后台任务】【光标移动】移动列表选择并重置日志视口。
    /// 参数: delta 为相对行数；返回: 无。
    pub fn move_selection(&mut self, delta: isize) {
        let visible = self.visible();
        let current = visible
            .iter()
            .position(|index| Some(&self.tasks[*index].id) == self.selected_id.as_ref())
            .unwrap_or(0);
        let next = current
            .saturating_add_signed(delta)
            .min(visible.len().saturating_sub(1));
        self.selected_id = visible.get(next).map(|index| self.tasks[*index].id.clone());
        self.offset = 0;
        self.follow = true;
    }

    /// 【后台任务】【范围切换】变更筛选并修复选择，不保留其他范围的日志模式。
    /// 参数: filter 为目标范围；返回: 无。
    pub fn set_filter(&mut self, filter: Filter) {
        self.filter = filter;
        self.reconcile();
    }

    /// 【后台任务】【状态合并】应用请求结果，只将匹配当前任务的输出展示出来。
    /// 参数: update 为服务端最新状态；返回: 无。
    pub fn apply(&mut self, update: Update) {
        self.tasks = update.tasks;
        self.tasks.sort_by(|a, b| {
            (b.status == "running")
                .cmp(&(a.status == "running"))
                .then(b.started_at.cmp(&a.started_at))
                .then(a.id.cmp(&b.id))
        });
        self.reconcile();
        if let Some((id, output)) = update.output {
            if Some(&id) == self.selected_id.as_ref() {
                if self.output_id.as_ref() != Some(&id) {
                    self.output = Output::default();
                }
                self.output_id = Some(id);
                match output {
                    Ok(output) => {
                        self.output = output;
                        self.output_error = None;
                    }
                    Err(error) => self.output_error = Some(error),
                }
            }
        }
        if let Some(notice) = update.notice {
            self.notify(notice, false);
        }
    }

    /// 【后台任务】【输出分流】获取当前缓冲中的日志行，合并模式按流分组而非假设时间顺序。
    /// 参数: 无；返回: 清除终端控制序列的日志行。
    pub fn log_lines(&self) -> Vec<String> {
        if self.output_id != self.selected_id {
            return Vec::new();
        }
        let mut result = Vec::new();
        for (stream, value) in [
            (Stream::Stdout, &self.output.stdout),
            (Stream::Stderr, &self.output.stderr),
        ] {
            if self.stream != Stream::All && self.stream != stream {
                continue;
            }
            for line in value.as_deref().unwrap_or_default().lines() {
                let prefix = if self.stream == Stream::All {
                    if stream == Stream::Stdout {
                        "[out] "
                    } else {
                        "[err] "
                    }
                } else {
                    ""
                };
                result.push(format!("{prefix}{}", safe_text(line)));
            }
        }
        result
    }

    /// 【后台任务】【视口定位】根据当前日志数计算有效偏移，尾随时停在最后一页。
    /// 参数: rows 为可见日志行数；返回: 实际首行偏移。
    pub fn pager_offset(&self, rows: usize) -> usize {
        let max = self.log_lines().len().saturating_sub(rows);
        if self.follow {
            max
        } else {
            self.offset.min(max)
        }
    }

    /// 【后台任务】【日志滚动】从当前可见位置滚动并暂停尾随，避免从尾部突然跳到开头。
    /// 参数: delta 为相对行数，rows 为视口高度；返回: 无。
    pub fn scroll(&mut self, delta: isize, rows: usize) {
        let max = self.log_lines().len().saturating_sub(rows);
        self.offset = self
            .pager_offset(rows)
            .saturating_add_signed(delta)
            .min(max);
        self.follow = false;
    }

    /// 【后台任务】【通道切换】循环切换标准输出、错误输出和按流分组输出。
    /// 参数: 无；返回: 无。
    pub fn cycle_stream(&mut self) {
        self.stream = match self.stream {
            Stream::Stdout => Stream::Stderr,
            Stream::Stderr => Stream::All,
            Stream::All => Stream::Stdout,
        };
        self.offset = 0;
        self.follow = true;
    }

    /// 【后台任务】【结果提示】保存操作反馈，错误持续保留直到下一次操作。
    /// 参数: message 为提示文本，error 为错误标识；返回: 无。
    pub fn notify(&mut self, message: String, error: bool) {
        self.notice = Some((
            safe_text(&message),
            error,
            Instant::now() + Duration::from_secs(4),
        ));
    }
}

/// 【后台任务】【文本安全】移除 ANSI、控制字符及换行，避免日志和任务标签控制终端。
/// 参数: value 为原始文本；返回: 可安全置于单行的文本。
pub(super) fn safe_text(value: &str) -> String {
    crate::render::ansi_clip::strip_ansi(value)
        .chars()
        .filter_map(|ch| {
            if matches!(ch, '\t' | '\n' | '\r') {
                Some(' ')
            } else if ch.is_control() {
                None
            } else {
                Some(ch)
            }
        })
        .collect()
}
