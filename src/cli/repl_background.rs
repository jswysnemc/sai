use crate::config::AppConfig;
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use crate::tools::command::{
    cleanup_background_tasks_for_user, list_background_tasks_for_user,
    read_background_task_output_for_user, stop_background_task_for_user,
};
use anyhow::Result;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::style::Print;
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, queue};
use serde::Deserialize;
use std::io::{self, Write};

const OUTPUT_TAIL_LINES: usize = 200;

#[derive(Debug, Clone, Deserialize)]
struct ReplBackgroundTask {
    id: String,
    label: String,
    command: String,
    cwd: String,
    pid: u32,
    status: String,
    started_at: u64,
    updated_at: u64,
    timeout_seconds: u64,
}

#[derive(Debug, Deserialize)]
struct ReplBackgroundTaskList {
    tasks: Vec<ReplBackgroundTask>,
}

#[derive(Debug, Deserialize)]
struct ReplBackgroundOutput {
    stdout: Option<String>,
    stderr: Option<String>,
}

struct ReplBackgroundScreen {
    stdout: io::Stdout,
}

impl ReplBackgroundScreen {
    /// 进入 REPL 后台任务管理屏幕。
    ///
    /// 返回:
    /// - 后台任务屏幕会话
    fn start() -> Result<Self> {
        let mut stdout = io::stdout();
        terminal::enable_raw_mode()?;
        execute!(stdout, EnterAlternateScreen, Hide)?;
        Ok(Self { stdout })
    }
}

impl Drop for ReplBackgroundScreen {
    fn drop(&mut self) {
        let _ = execute!(self.stdout, Show, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}

/// 运行 REPL 后台任务交互管理界面。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 应用配置
///
/// 返回:
/// - 交互管理是否成功
pub(super) async fn run_repl_background_manager(
    paths: &SaiPaths,
    config: &AppConfig,
) -> Result<()> {
    let mut screen = ReplBackgroundScreen::start()?;
    let mut tasks = load_repl_background_tasks(paths, config).await?;
    let mut selected = 0usize;
    let mut status = String::new();
    loop {
        selected = clamp_selected(selected, tasks.len());
        draw_task_list(&mut screen.stdout, &tasks, selected, &status)?;
        if let Event::Key(KeyEvent {
            code, modifiers, ..
        }) = event::read()?
        {
            match code {
                KeyCode::Esc | KeyCode::Char('q') => break,
                KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => break,
                KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => {
                    selected = (selected + 1).min(tasks.len().saturating_sub(1));
                }
                KeyCode::Char('r') => {
                    tasks = load_repl_background_tasks(paths, config).await?;
                    status = t("refreshed", "已刷新").to_string();
                }
                KeyCode::Enter | KeyCode::Char('o') => {
                    if let Some(task) = tasks.get(selected) {
                        show_task_output(&mut screen.stdout, paths, config, task).await?;
                    }
                }
                KeyCode::Char('s') => {
                    if let Some(task) = tasks.get(selected) {
                        stop_background_task_for_user(paths, config, &task.id, false).await?;
                        status = format!("{}: {}", t("stopped", "已停止"), task.id);
                        tasks = load_repl_background_tasks(paths, config).await?;
                    }
                }
                KeyCode::Char('f') => {
                    if let Some(task) = tasks.get(selected) {
                        stop_background_task_for_user(paths, config, &task.id, true).await?;
                        status = format!("{}: {}", t("force stopped", "已强制停止"), task.id);
                        tasks = load_repl_background_tasks(paths, config).await?;
                    }
                }
                KeyCode::Char('x') => {
                    cleanup_background_tasks_for_user(paths, config, false).await?;
                    status = t("finished tasks cleaned", "已清理结束任务").to_string();
                    tasks = load_repl_background_tasks(paths, config).await?;
                }
                _ => {}
            }
        }
    }
    Ok(())
}

/// 加载 REPL 后台任务列表。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `config`: 应用配置
///
/// 返回:
/// - 后台任务列表
async fn load_repl_background_tasks(
    paths: &SaiPaths,
    config: &AppConfig,
) -> Result<Vec<ReplBackgroundTask>> {
    let raw = list_background_tasks_for_user(paths, config).await?;
    let list: ReplBackgroundTaskList = serde_json::from_str(&raw)?;
    Ok(list.tasks)
}

/// 绘制后台任务列表。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `tasks`: 后台任务列表
/// - `selected`: 当前选中索引
/// - `status`: 操作状态文本
///
/// 返回:
/// - 绘制是否成功
fn draw_task_list(
    stdout: &mut io::Stdout,
    tasks: &[ReplBackgroundTask],
    selected: usize,
    status: &str,
) -> Result<()> {
    queue!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
    draw_line(stdout, t("background tasks", "后台任务"))?;
    draw_line(
        stdout,
        t(
            "Up/Down select, Enter output, s stop, f force stop, r refresh, x cleanup, Esc return",
            "Up/Down 选择，Enter 输出，s 停止，f 强制停止，r 刷新，x 清理，Esc 返回",
        ),
    )?;
    draw_line(stdout, "")?;
    if tasks.is_empty() {
        draw_line(stdout, t("no background tasks", "暂无后台任务"))?;
    } else {
        for (index, task) in tasks.iter().enumerate() {
            let marker = if index == selected { ">" } else { " " };
            let line = format!(
                "{marker} {:<10} {:<10} pid={:<8} {}",
                task.id,
                task.status,
                task.pid,
                truncate_text(&task.label, 48)
            );
            draw_line(stdout, &line)?;
        }
    }
    draw_line(stdout, "")?;
    if let Some(task) = tasks.get(selected) {
        draw_task_detail(stdout, task)?;
    }
    if !status.trim().is_empty() {
        draw_line(stdout, "")?;
        draw_line(stdout, status)?;
    }
    stdout.flush()?;
    Ok(())
}

/// 绘制后台任务详情。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `task`: 后台任务
///
/// 返回:
/// - 绘制是否成功
fn draw_task_detail(stdout: &mut io::Stdout, task: &ReplBackgroundTask) -> Result<()> {
    draw_line(stdout, &format!("{}: {}", t("id", "ID"), task.id))?;
    draw_line(stdout, &format!("{}: {}", t("cwd", "目录"), task.cwd))?;
    draw_line(
        stdout,
        &format!("{}: {}", t("command", "命令"), task.command),
    )?;
    draw_line(
        stdout,
        &format!(
            "{}: started={} updated={} timeout={}s",
            t("time", "时间"),
            task.started_at,
            task.updated_at,
            task.timeout_seconds
        ),
    )?;
    Ok(())
}

/// 展示后台任务输出。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `paths`: Sai 路径
/// - `config`: 应用配置
/// - `task`: 后台任务
///
/// 返回:
/// - 展示是否成功
async fn show_task_output(
    stdout: &mut io::Stdout,
    paths: &SaiPaths,
    config: &AppConfig,
    task: &ReplBackgroundTask,
) -> Result<()> {
    let raw =
        read_background_task_output_for_user(paths, config, &task.id, "all", OUTPUT_TAIL_LINES)
            .await?;
    let output: ReplBackgroundOutput = serde_json::from_str(&raw)?;
    loop {
        queue!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
        draw_line(
            stdout,
            &format!(
                "{}: {}  {}",
                t("output", "输出"),
                task.id,
                t("Esc return", "Esc 返回")
            ),
        )?;
        draw_line(stdout, "")?;
        draw_output_block(
            stdout,
            "stdout",
            output.stdout.as_deref().unwrap_or_default(),
        )?;
        draw_line(stdout, "")?;
        draw_output_block(
            stdout,
            "stderr",
            output.stderr.as_deref().unwrap_or_default(),
        )?;
        stdout.flush()?;
        if let Event::Key(KeyEvent { code, .. }) = event::read()? {
            if matches!(code, KeyCode::Esc | KeyCode::Char('q')) {
                break;
            }
        }
    }
    Ok(())
}

/// 绘制单个输出流。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `title`: 输出流标题
/// - `text`: 输出流文本
///
/// 返回:
/// - 绘制是否成功
fn draw_output_block(stdout: &mut io::Stdout, title: &str, text: &str) -> Result<()> {
    draw_line(stdout, &format!("--- {title} ---"))?;
    if text.trim().is_empty() {
        draw_line(stdout, t("(empty)", "（空）"))?;
        return Ok(());
    }
    for line in text.lines().take(200) {
        draw_line(stdout, line)?;
    }
    Ok(())
}

/// 绘制单行终端文本。
///
/// 参数:
/// - `stdout`: 终端输出
/// - `text`: 行文本
///
/// 返回:
/// - 绘制是否成功
fn draw_line(stdout: &mut io::Stdout, text: &str) -> Result<()> {
    queue!(stdout, Print(text), Print("\r\n"))?;
    Ok(())
}

/// 修正当前选中索引。
///
/// 参数:
/// - `selected`: 当前选中索引
/// - `len`: 列表长度
///
/// 返回:
/// - 合法选中索引
fn clamp_selected(selected: usize, len: usize) -> usize {
    selected.min(len.saturating_sub(1))
}

/// 截断过长文本。
///
/// 参数:
/// - `text`: 原始文本
/// - `max_chars`: 最大字符数
///
/// 返回:
/// - 截断后的文本
fn truncate_text(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let mut value = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        value.push_str("...");
    }
    value
}
