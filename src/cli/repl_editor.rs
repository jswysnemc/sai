use anyhow::{bail, Context, Result};
use std::fs;
use std::path::PathBuf;
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
    fs::write(&path, input).with_context(|| format!("failed to write {}", path.display()))?;
    let status = crate::platform::shell::editor_command(&editor, &path)
        .status()
        .with_context(|| format!("failed to launch editor: {editor}"))?;
    if !status.success() {
        let _ = fs::remove_file(&path);
        bail!("editor exited with status: {status}");
    }
    let edited =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let _ = fs::remove_file(&path);
    Ok(edited.trim_end_matches(['\r', '\n']).to_string())
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
