use crate::config::AppConfig;
use crate::llm::{ChatMessage, ChatStreamEvent, OpenAiCompatibleClient};
use crate::paths::SaiPaths;
use anyhow::{bail, Result};
use std::path::Path;
use std::time::Duration;
use tokio::time::timeout;

const COMMIT_TIMEOUT: Duration = Duration::from_secs(45);

/// 为提交说明生成构造客户端。
///
/// 参数:
/// - `config`: 应用配置
/// - `paths`: Sai 路径
///
/// 返回:
/// - 提交说明客户端
pub(crate) fn resolve_commit_message_client(
    config: &AppConfig,
    paths: &SaiPaths,
) -> Result<OpenAiCompatibleClient> {
    let runtime = commit_runtime_config(config)?;
    OpenAiCompatibleClient::from_config(&runtime, paths)
}

fn commit_runtime_config(config: &AppConfig) -> Result<AppConfig> {
    let provider_id = config.git.auto_commit_message_provider_id.trim();
    let model = config.git.auto_commit_message_model.trim();
    match (provider_id.is_empty(), model.is_empty()) {
        (true, true) => Ok(config.clone()),
        (false, false) => {
            let mut runtime = config.clone();
            runtime.set_active_provider_model(provider_id, model)?;
            Ok(runtime)
        }
        _ => bail!(
            "git.auto_commit_message_provider_id and git.auto_commit_message_model must be provided together"
        ),
    }
}

/// 根据 diff / 状态摘要生成 Conventional Commits 风格说明。
///
/// 参数:
/// - `client`: 模型客户端
/// - `status_summary`: `git status` 风格摘要
/// - `diff_summary`: 已暂存或工作区 diff 摘要
///
/// 返回:
/// - 提交说明正文
pub(crate) async fn generate_commit_message(
    client: &OpenAiCompatibleClient,
    status_summary: &str,
    diff_summary: &str,
) -> Result<String> {
    let user = format!(
        "Git status:\n{}\n\nDiff summary:\n{}\n",
        truncate(status_summary, 2500),
        truncate(diff_summary, 12000)
    );
    let messages = vec![
        ChatMessage::system(
            "You write Git commit messages. Output ONLY the commit message body using Conventional Commits (type(scope): subject). Prefer Chinese subject when the change descriptions are Chinese. Keep subject under 72 characters. Optionally add a short body after a blank line. No markdown fences, no quotes, no commentary.",
        ),
        ChatMessage::plain("user", user),
    ];
    let result = match timeout(
        COMMIT_TIMEOUT,
        client.chat_stream_events(messages, Vec::new(), |_event: ChatStreamEvent| Ok(())),
    )
    .await
    {
        Ok(Ok(result)) => result,
        Ok(Err(error)) => return Err(error),
        Err(_) => bail!("commit message generation timed out"),
    };
    Ok(sanitize_commit_message(&result.content))
}

/// 从仓库收集状态与 diff 摘要供模型使用。
///
/// 参数:
/// - `repo`: 仓库根目录
///
/// 返回:
/// - (status, diff) 文本
pub(crate) async fn collect_repo_change_summary(repo: &Path) -> Result<(String, String)> {
    let status = run_git(repo, &["status", "--short", "--branch"]).await?;
    let staged = run_git(repo, &["diff", "--cached", "--stat", "--patch"])
        .await
        .unwrap_or_default();
    let diff = if staged.trim().is_empty() {
        run_git(repo, &["diff", "--stat", "--patch"])
            .await
            .unwrap_or_default()
    } else {
        staged
    };
    Ok((status, diff))
}

async fn run_git(repo: &Path, args: &[&str]) -> Result<String> {
    let output = tokio::process::Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("git {} failed: {}", args.join(" "), stderr.trim());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn sanitize_commit_message(raw: &str) -> String {
    let text = raw
        .trim()
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    let mut lines = text.lines().map(str::trim_end).collect::<Vec<_>>();
    if lines
        .first()
        .is_some_and(|line| matches!(line.trim(), "text" | "markdown" | "md" | "commit"))
    {
        lines.remove(0);
    }
    let body = lines.join("\n").trim().to_string();
    truncate(&body, 2000)
}

fn truncate(text: &str, max_chars: usize) -> String {
    let count = text.chars().count();
    if count <= max_chars {
        return text.to_string();
    }
    text.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_fences() {
        let msg = sanitize_commit_message("```\nfeat: add retry\n```");
        assert_eq!(msg, "feat: add retry");
    }
}
