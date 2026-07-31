use super::{AcpEngine, AcpGovernance};
use crate::config::AppConfig;
use crate::paths::SaiPaths;
use anyhow::{Context, Result};
use std::time::Duration;

/// 预热结果：外部内核在握手中公布的身份。
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct AcpWarmupOutcome {
    /// agent 自报名称
    pub(crate) agent: String,
    /// agent 自报版本
    pub(crate) version: String,
}

/// 【ACP】【预热连接】主动连接外部内核并抓取其运行时能力。
///
/// 对话内核默认延迟启动，首轮对话之前界面无法得知 agent 支持哪些模型与
/// 思考等级。本入口拉起进程走完握手与建会话，把能力写入全局运行状态后
/// 立即回收进程，让界面在开始对话前就能给出可选项。
///
/// 参数:
/// - `config`: 当前应用配置，提供内核标识与启动参数
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 握手成功时返回 agent 身份；内核非外部或启动失败时返回错误
pub(crate) async fn warm_up(config: &AppConfig, paths: &SaiPaths) -> Result<AcpWarmupOutcome> {
    // 1. 仅外部内核需要预热，自带内核没有握手过程
    if !config.agent.engine.is_external() {
        anyhow::bail!(crate::i18n::text(
            "the current agent engine is not external",
            "当前对话内核不是外部内核"
        ));
    }
    let (program, args) = config
        .agent
        .resolved_command()
        .context("agent engine has no launch command")?;
    let workspace =
        crate::runtime_cwd::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let governance = AcpGovernance::for_warmup(workspace.clone(), config.clone(), paths);
    // 2. 握手与建会话；会话配置项在建会话后才会写入全局状态
    let mut engine = AcpEngine::connect(
        config.agent.engine.as_str(),
        &program,
        &args,
        &config.agent.acp.env,
        &workspace,
        Duration::from_secs(config.agent.acp.startup_timeout_seconds),
        governance,
    )
    .await?;
    let info = engine.warm_up_runtime_state(&workspace).await?;
    let (agent, version) =
        info.unwrap_or_else(|| (config.agent.engine.as_str().to_string(), String::new()));
    Ok(AcpWarmupOutcome { agent, version })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【ACP】【预热连接】验证自带内核在拉起进程之前就被拒绝。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[tokio::test]
    async fn warm_up_rejects_native_engine() {
        let temp = tempfile::tempdir().expect("临时目录");
        let root = temp.path();
        let paths = SaiPaths {
            config_dir: root.join("config"),
            config_file: root.join("config/config.jsonc"),
            secrets_file: root.join("config/secrets.jsonc"),
            skills_dir: root.join("config/skills"),
            data_dir: root.join("data"),
            cache_dir: root.join("cache"),
            state_dir: root.join("state"),
            pictures_dir: root.join("pictures"),
            fish_hook_file: root.join("fish/sai.fish"),
            bash_hook_file: root.join("shell/bash-hook.sh"),
            zsh_hook_file: root.join("shell/zsh-hook.zsh"),
            powershell_hook_file: root.join("shell/powershell-hook.ps1"),
        };
        let config = AppConfig::default();
        assert!(!config.agent.engine.is_external(), "默认配置应使用自带内核");

        let error = warm_up(&config, &paths)
            .await
            .expect_err("自带内核应被拒绝");

        let message = error.to_string();
        assert!(
            message.contains("not external") || message.contains("不是外部内核"),
            "错误应说明内核类型: {message}"
        );
    }
}
