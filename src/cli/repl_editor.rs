use crate::i18n::text as t;
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::PathBuf;
#[cfg(windows)]
use std::time::{Duration, Instant};
use std::time::{SystemTime, UNIX_EPOCH};

/// 使用用户配置的编辑器编辑 REPL 输入缓冲区。
///
/// 参数:
/// - `input`: 当前输入缓冲区
///
/// 返回:
/// - 编辑后的输入内容
pub(super) fn edit_input_buffer(input: &str) -> Result<String> {
    let editor = std::env::var("VISUAL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var("EDITOR")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(|| crate::platform::shell::default_editor().to_string());
    let path = temporary_buffer_path();
    fs::write(&path, input)
        .with_context(|| format!("{} {}", t("failed to write", "写入失败"), path.display()))?;
    #[cfg(windows)]
    let before = fs::metadata(&path)
        .and_then(|metadata| metadata.modified())
        .unwrap_or_else(|_| SystemTime::now());
    #[cfg(windows)]
    let launch_started = Instant::now();
    let status = crate::platform::shell::editor_command(&editor, &path)
        .status()
        .with_context(|| {
            format!(
                "{}: {editor}",
                t("failed to launch editor", "无法启动编辑器")
            )
        })?;
    if !status.success() {
        let _ = fs::remove_file(&path);
        bail!(
            "{}: {status}",
            t("editor exited with status", "编辑器退出状态")
        );
    }
    #[cfg(windows)]
    {
        // 1. shell 转发 GUI 编辑器时可能立即返回，保存发生在其后；
        //    仅对快速返回的 notepad 等命令等待保存，正常阻塞退出不额外延迟
        wait_for_gui_editor_save(&editor, &path, before, launch_started.elapsed());
    }
    let edited = fs::read_to_string(&path)
        .with_context(|| format!("{} {}", t("failed to read", "读取失败"), path.display()))?;
    let _ = fs::remove_file(&path);
    Ok(edited.trim_end_matches(['\r', '\n']).to_string())
}

/// Windows 下等待非阻塞 GUI 编辑器保存临时文件。
///
/// 新版 notepad 把文件转发给既有窗口后进程立即退出，调用方无法用退出码
/// 判断编辑是否完成；这里轮询文件修改时间，保存发生后立即返回，
/// 用户在终端按 Enter 可提前结束等待，未保存关闭时不至于卡住。
///
/// 参数:
/// - `editor`: 用户配置的编辑器命令
/// - `path`: 临时文件路径
/// - `before`: 启动编辑器前的文件修改时间
/// - `elapsed`: 编辑器进程调用返回所耗时长
///
/// 返回:
/// - 无
#[cfg(windows)]
fn wait_for_gui_editor_save(editor: &str, path: &PathBuf, before: SystemTime, elapsed: Duration) {
    let name = editor_executable_name(editor);
    if !name.starts_with("notepad") || elapsed > Duration::from_secs(2) {
        return;
    }
    if modified_after(path, before) {
        return;
    }
    eprintln!(
        "{}",
        t(
            "waiting for the editor to save; press Enter here to continue without saving",
            "等待编辑器保存；若未保存，回到终端按 Enter 继续",
        )
    );
    // 1. 轮询保存事件；终端有按键（如 Enter）时提前结束，未保存关闭不至于卡住
    let deadline = Instant::now() + Duration::from_secs(300);
    while Instant::now() < deadline {
        if modified_after(path, before) {
            return;
        }
        if crossterm::event::poll(Duration::from_millis(250)).unwrap_or(false) {
            // 2. 只把 Enter 视为放弃等待，忽略窗口调整等非按键事件
            if matches!(
                crossterm::event::read(),
                Ok(crossterm::event::Event::Key(key))
                    if key.code == crossterm::event::KeyCode::Enter
                        && key.kind != crossterm::event::KeyEventKind::Release
            ) {
                return;
            }
        }
    }
}

/// 提取 Windows 编辑器命令的可执行文件名。
///
/// 参数:
/// - `editor`: 用户配置的编辑器命令，可包含引号和附加参数
///
/// 返回:
/// - 小写可执行文件名
#[cfg(windows)]
fn editor_executable_name(editor: &str) -> String {
    let command = editor.trim();
    let head = command
        .strip_prefix('"')
        .and_then(|value| value.split_once('"').map(|(path, _)| path))
        .or_else(|| {
            command
                .strip_prefix('\'')
                .and_then(|value| value.split_once('\'').map(|(path, _)| path))
        })
        .or_else(|| command.split_whitespace().next())
        .unwrap_or_default();
    head.rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
}

/// 判断文件修改时间是否晚于给定时刻。
///
/// 参数:
/// - `path`: 临时文件路径
/// - `before`: 基准修改时间
///
/// 返回:
/// - 文件仍存在且修改时间更晚时返回 true
#[cfg(windows)]
fn modified_after(path: &PathBuf, before: SystemTime) -> bool {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .map(|modified| modified > before)
        .unwrap_or(false)
}

/// 生成 REPL 编辑临时文件路径。
///
/// 返回:
/// - 临时文件路径
fn temporary_buffer_path() -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    std::env::temp_dir().join(format!("sai-repl-{timestamp}.md"))
}
