use super::*;

/// 【终端】【模型选择】保存后同步运行时，同时保留仅修改子任务时的主对话思考选择。
///
/// 参数: `paths` 为存储路径，其余为当前配置、客户端、Agent、界面、模式和命令行思考覆盖
/// 返回: 交互与重载结果，用户取消时正常返回
pub(super) fn run_picker(
    paths: &SaiPaths,
    config: &mut AppConfig,
    client: &mut OpenAiCompatibleClient,
    agent: &mut Agent,
    runtime: &mut ReplRuntime,
    mode: AgentMode,
    thinking_override: &mut Option<String>,
) -> Result<()> {
    let outcome = models_picker::run_interactive(paths);
    runtime.redraw()?;
    match outcome {
        Ok(models_picker::PickerOutcome::Cancelled) => {
            runtime.record_meta(t("model selection cancelled", "已取消模型选择").to_string())?;
        }
        Ok(models_picker::PickerOutcome::Saved {
            message,
            main_model_changed,
        }) => {
            if main_model_changed {
                *thinking_override = None;
            }
            reload_repl_agent(
                paths,
                config,
                client,
                agent,
                mode,
                thinking_override.as_deref(),
            )?;
            runtime.record_meta(message)?;
        }
        Err(error) => runtime.record_meta(error.to_string())?,
    }
    Ok(())
}
