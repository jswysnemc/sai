use super::*;

/// 【终端】【会话模式】处理权限模式命令和 `/code-model`。
///
/// 参数: input 为提交文本，mode 为当前会话模式，runtime 为终端界面，paths 为配置路径，state_dir 为会话目录
/// 返回: 命中模式命令时返回 true；保存失败只展示提示并保留原模式
pub(super) fn handle(
    input: &str,
    mode: &mut AgentMode,
    runtime: &mut ReplRuntime,
    paths: &SaiPaths,
    state_dir: &std::path::Path,
) -> Result<bool> {
    let Some(next) = mode.command(input) else {
        return Ok(false);
    };
    let saved = match crate::cli::codemode_preferences::save_default_for_command(paths, input, next)
    {
        Ok(saved) => saved,
        Err(error) => {
            runtime.record_meta(format!(
                "{}: {error}",
                t("Failed to save tool mode", "工具方式保存失败")
            ))?;
            return Ok(true);
        }
    };
    if next.permission() == crate::agent::PermissionMode::Plan
        && mode.permission() != crate::agent::PermissionMode::Plan
    {
        crate::plan::store::begin_sync(state_dir, *mode)?;
    }
    *mode = next;
    runtime.record_meta(format!("{}: {}", t("mode", "模式"), mode.label()))?;
    if saved {
        runtime.record_meta(t("Default tool mode saved", "默认工具方式已保存").to_string())?;
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CodeMode;
    use crate::render::transcript::TranscriptRenderOptions;

    /// 【工具方式】【测试界面】创建采用摘要展示的终端运行期。
    /// 参数: 无；返回: 独立测试界面。
    fn runtime() -> ReplRuntime {
        ReplRuntime::new(
            5000,
            TranscriptRenderOptions {
                reasoning_mode: render::ReasoningDisplayMode::Summary,
                tool_call_mode: render::ToolCallDisplayMode::Summary,
            },
        )
    }

    /// 【工具方式】【空闲保存】命令切换当前模式并保存默认值，不改变当前权限。
    /// 参数: 无；返回: 无。
    #[test]
    fn idle_command_saves_selected_tool_mode() {
        let root = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(root.path());
        let mut runtime = runtime();
        let mut mode = AgentMode::Audited;
        assert!(handle(
            "/code-model only",
            &mut mode,
            &mut runtime,
            &paths,
            root.path(),
        )
        .unwrap());
        assert_eq!(mode, AgentMode::Audited.with_code_mode(CodeMode::Only));
        assert_eq!(
            AppConfig::load(&paths).unwrap().codemode.mode,
            CodeMode::Only
        );
    }

    /// 【工具方式】【空闲失败】保存失败只展示提示，不终止输入循环或改变会话模式。
    /// 参数: 无；返回: 无。
    #[test]
    fn idle_command_keeps_mode_when_saving_fails() {
        let root = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(root.path());
        paths.create_dirs().unwrap();
        std::fs::create_dir(&paths.config_file).unwrap();
        let mut runtime = runtime();
        let mut mode = AgentMode::Audited;
        assert!(handle(
            "/code-model on",
            &mut mode,
            &mut runtime,
            &paths,
            root.path(),
        )
        .unwrap());
        assert_eq!(mode, AgentMode::Audited);
    }
}
