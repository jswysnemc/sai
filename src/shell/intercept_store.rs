use crate::paths::SaiPaths;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const LAST_COMMAND_FILE: &str = "last-shell-command.json";

/// 最近一次由 shell hook 拦截的命令。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct StoredShellCommand {
    pub(crate) shell: String,
    pub(crate) command: String,
    pub(crate) clipb: bool,
    pub(crate) web_search: bool,
}

/// 保存最近一次 shell 拦截命令。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `shell`: 产生拦截的 shell 名称
/// - `command`: 原始命令文本
/// - `clipb`: 是否请求读取剪贴板
/// - `web_search`: 是否请求网络搜索
///
/// 返回:
/// - 保存后的命令记录
pub(crate) fn store(
    paths: &SaiPaths,
    shell: &str,
    command: &str,
    clipb: bool,
    web_search: bool,
) -> Result<StoredShellCommand> {
    let record = StoredShellCommand {
        shell: shell.to_string(),
        command: command.trim().to_string(),
        clipb,
        web_search,
    };
    std::fs::create_dir_all(&paths.state_dir)?;
    let content = serde_json::to_vec_pretty(&record)?;
    let temp = paths.state_dir.join(format!("{LAST_COMMAND_FILE}.tmp"));
    std::fs::write(&temp, content)?;
    std::fs::rename(&temp, path(paths))?;
    Ok(record)
}

/// 读取最近一次 shell 拦截命令。
///
/// 参数:
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 已保存命令，不存在时返回空
pub(crate) fn load(paths: &SaiPaths) -> Result<Option<StoredShellCommand>> {
    let file = path(paths);
    if !file.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&file)
        .with_context(|| format!("failed to read {}", file.display()))?;
    Ok(Some(serde_json::from_str(&content)?))
}

/// 将最近命令转换为对话上下文。
///
/// 参数:
/// - `record`: 最近一次命令
/// - `instruction`: 用户补充说明
///
/// 返回:
/// - 可直接发送给模型的消息
pub(crate) fn prompt(record: &StoredShellCommand, instruction: &str) -> String {
    let instruction = instruction.trim();
    let mut message = format!(
        "Explain or help fix this command from {}:\n```{}\n{}\n```",
        record.shell, record.shell, record.command
    );
    if !instruction.is_empty() {
        message.push_str("\n\nUser request: ");
        message.push_str(instruction);
    }
    message
}

fn path(paths: &SaiPaths) -> PathBuf {
    paths.state_dir.join(LAST_COMMAND_FILE)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(root: &std::path::Path) -> SaiPaths {
        SaiPaths {
            config_dir: root.join("config"),
            config_file: root.join("config.jsonc"),
            secrets_file: root.join("secrets.jsonc"),
            skills_dir: root.join("skills"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            state_dir: root.join("state"),
            pictures_dir: root.join("pictures"),
            fish_hook_file: root.join("sai.fish"),
            bash_hook_file: root.join("bash-hook.sh"),
            zsh_hook_file: root.join("zsh-hook.zsh"),
            powershell_hook_file: root.join("powershell-hook.ps1"),
        }
    }

    #[test]
    fn stores_and_reads_last_command_atomically() {
        let temp = tempfile::tempdir().unwrap();
        let paths = paths(temp.path());
        let stored = store(&paths, "zsh", "source ~/.config/zsh/.zshrcx", true, true).unwrap();

        assert_eq!(load(&paths).unwrap(), Some(stored));
        assert!(path(&paths).is_file());
    }

    #[test]
    fn builds_prompt_with_optional_instruction() {
        let record = StoredShellCommand {
            shell: "zsh".to_string(),
            command: "missing command".to_string(),
            clipb: false,
            web_search: false,
        };
        let prompt = prompt(&record, "explain the error");

        assert!(prompt.contains("from zsh"));
        assert!(prompt.contains("missing command"));
        assert!(prompt.contains("explain the error"));
    }
}
