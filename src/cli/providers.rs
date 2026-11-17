use super::*;

/// 【供应商命令】【列表与切换】展示已激活供应商模型或按序号/交互切换当前默认模型。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `args`: 供应商命令参数
///
/// 返回:
/// - 执行是否成功
pub(super) fn run_providers(paths: &SaiPaths, args: ProvidersArgs) -> Result<()> {
    let mut config = AppConfig::load(paths)?;
    let choices = config.provider_model_choices();
    if choices.is_empty() {
        bail!(
            "{}",
            t(
                "no active provider models; configure or activate a model first",
                "没有已激活的供应商模型；请先配置或激活模型",
            )
        );
    }
    if let Some(index) = args.index {
        if index == 0 || index > choices.len() {
            bail!(
                "{}: {index}",
                t("provider index out of range", "供应商序号超出范围")
            );
        }
        let choice = &choices[index - 1];
        let provider_id = choice.provider_id.clone();
        let model = choice.model.clone();
        let label = choice.label();
        config.set_active_provider_model(&provider_id, &model)?;
        config.save(paths)?;
        println!(
            "{}: {index}. {label}",
            t("active provider", "当前供应商")
        );
        return Ok(());
    }
    if io::stdout().is_terminal() && io::stdin().is_terminal() {
        if let Some(index) = inline_fuzzy_select(
            &choices
                .iter()
                .map(|choice| choice.label())
                .collect::<Vec<_>>(),
        )? {
            let choice = &choices[index];
            let provider_id = choice.provider_id.clone();
            let model = choice.model.clone();
            let label = choice.label();
            config.set_active_provider_model(&provider_id, &model)?;
            config.save(paths)?;
            println!(
                "{}: {}. {label}",
                t("active provider", "当前供应商"),
                index + 1
            );
        }
        return Ok(());
    }
    for (index, choice) in choices.iter().enumerate() {
        let active = config
            .provider(None)
            .map(|provider| {
                provider.id == choice.provider_id && provider.default_model == choice.model
            })
            .unwrap_or(false);
        let marker = if active { "*" } else { " " };
        println!("{marker} {}. {}", index + 1, choice.label());
    }
    Ok(())
}

/// 【配置设置】【命令分发】执行配置设置子命令。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `args`: set 子命令参数
///
/// 返回:
/// - 设置是否成功
pub(super) fn run_set(paths: &SaiPaths, args: SetArgs) -> Result<()> {
    match args.command {
        SetCommand::Thinking(args) => run_set_thinking(paths, args),
    }
}

/// 【思考等级】【CLI入口】设置当前供应商的思考等级并在标准输出打印结果。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `args`: thinking 参数
///
/// 返回:
/// - 设置是否成功
pub(super) fn run_set_thinking(paths: &SaiPaths, args: SetThinkingArgs) -> Result<()> {
    let message = update_thinking_level(paths, args.level.as_deref(), true)?;
    println!("{message}");
    Ok(())
}

/// 【思考等级】【静默更新】解析或交互选择思考等级并持久化，不向受管屏幕直接输出换行。
///
/// 参数:
/// - `paths`: Sai 路径集合
/// - `level`: 可选目标思考等级，为空时打开内联选择器
/// - `print_current_in_cli`: CLI 交互模式下是否在选择前打印当前等级提示
///
/// 返回:
/// - 更新完成后的双语状态提示文本
pub(in crate::cli) fn update_thinking_level(
    paths: &SaiPaths,
    level: Option<&str>,
    print_current_in_cli: bool,
) -> Result<String> {
    // 1. 【思考等级】【读取供应商】定位当前激活供应商及其展示等级
    AppConfig::init_files(paths)?;
    let mut config = AppConfig::load(paths)?;
    let active = config.active_provider.clone();
    let provider = config
        .providers
        .iter_mut()
        .find(|provider| provider.id == active)
        .ok_or_else(|| {
            anyhow::anyhow!("{}: {active}", t("unknown provider", "未知供应商"))
        })?;
    let current_level = display_thinking_level(&provider.thinking_level);
    let provider_name = provider.display_name.clone();
    // 2. 【思考等级】【解析与保存】统一将 auto 存储为空串以保持供应商默认行为语义
    let resolved = resolve_thinking_level_arg(
        level,
        &current_level,
        &provider_name,
        print_current_in_cli,
    )?;
    provider.thinking_level = stored_thinking_level(resolved);
    config.save(paths)?;
    Ok(format!(
        "{}: {} / {}",
        t("thinking level", "思考等级"),
        provider_name,
        resolved
    ))
}

/// 【思考等级】【参数解析】解析显式等级或在交互终端弹出模糊选择。
///
/// 参数:
/// - `level`: 命令行或斜杠命令传入的思考等级
/// - `current_level`: 当前生效的展示等级
/// - `provider_name`: 当前供应商展示名称
/// - `print_current_in_cli`: 是否在 CLI 模式下输出当前等级标题
///
/// 返回:
/// - 归一化后的合法思考等级名称
fn resolve_thinking_level_arg(
    level: Option<&str>,
    current_level: &str,
    provider_name: &str,
    print_current_in_cli: bool,
) -> Result<&'static str> {
    if let Some(level) = level.map(str::trim).filter(|value| !value.is_empty()) {
        return normalize_thinking_level(level);
    }
    if !(io::stdout().is_terminal() && io::stdin().is_terminal()) {
        bail!(
            "{}",
            t(
                "thinking level is required in non-interactive mode",
                "非交互模式必须提供思考等级",
            )
        );
    }
    if print_current_in_cli {
        println!(
            "{}: {} / {}",
            t("current thinking level", "当前思考等级"),
            provider_name,
            current_level
        );
    }
    let labels = THINKING_LEVELS
        .iter()
        .map(|candidate| {
            if *candidate == current_level {
                format!("{candidate} ({})", t("current", "当前"))
            } else {
                (*candidate).to_string()
            }
        })
        .collect::<Vec<_>>();
    let Some(index) = inline_fuzzy_select(&labels)? else {
        bail!(
            "{}",
            t("thinking level selection cancelled", "已取消思考等级选择")
        );
    };
    Ok(THINKING_LEVELS[index])
}

/// 【思考等级】【持久化编码】将展示层思考等级转换为配置文件存储值，auto 统一存为空串。
///
/// 参数:
/// - `level`: 已归一化的思考等级
///
/// 返回:
/// - 写入配置的思考等级字符串
pub(in crate::cli) fn stored_thinking_level(level: &str) -> String {
    let trimmed = level.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("auto") {
        String::new()
    } else {
        trimmed.to_string()
    }
}

/// 【思考等级】【合法性校验】校验并归一化思考等级名称。
///
/// 参数:
/// - `level`: 原始思考等级文本
///
/// 返回:
/// - 归一化后的静态思考等级字符串
pub(super) fn normalize_thinking_level(level: &str) -> Result<&'static str> {
    let normalized = level.trim().to_ascii_lowercase();
    THINKING_LEVELS
        .iter()
        .copied()
        .find(|item| *item == normalized)
        .ok_or_else(|| anyhow::anyhow!("{}: {level}", t("invalid thinking level", "无效思考等级")))
}

/// 【思考等级】【展示格式化】返回用于界面展示的思考等级名称。
///
/// 参数:
/// - `level`: 配置中的原始思考等级
///
/// 返回:
/// - 非空思考等级，空值统一显示为 auto
pub(super) fn display_thinking_level(level: &str) -> String {
    let level = level.trim();
    if level.is_empty() {
        "auto".to_string()
    } else {
        level.to_string()
    }
}

/// 【思考等级】【临时覆盖】对当前激活供应商应用命令行临时思考等级覆盖。
///
/// 参数:
/// - `config`: 当前应用配置
/// - `level`: 命令行传入的思考等级
///
/// 返回:
/// - 等级非法或供应商不存在时返回错误
pub(super) fn apply_thinking_override(config: &mut AppConfig, level: Option<&str>) -> Result<()> {
    let Some(level) = level.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(());
    };
    let level = normalize_thinking_level(level)?;
    let active = config.active_provider.clone();
    let provider = config
        .providers
        .iter_mut()
        .find(|provider| provider.id == active)
        .ok_or_else(|| {
            anyhow::anyhow!("{}: {active}", t("unknown provider", "未知供应商"))
        })?;
    provider.thinking_level = stored_thinking_level(level);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【思考等级测试】【存储语义】验证 auto 与显式等级在存储、覆盖和展示层语义一致。
    #[test]
    fn auto_thinking_level_stores_as_empty_and_displays_as_auto() {
        let root = tempfile::tempdir().unwrap();
        let paths = SaiPaths::for_tests(root.path());
        let mut initial = AppConfig::default();
        initial.providers.truncate(1);
        initial.subagent = Default::default();
        initial.save(&paths).unwrap();

        let high_msg = update_thinking_level(&paths, Some("HIGH"), false).unwrap();
        assert!(high_msg.contains("high"));
        assert_eq!(
            AppConfig::load(&paths).unwrap().providers[0].thinking_level,
            "high"
        );

        let auto_msg = update_thinking_level(&paths, Some("auto"), false).unwrap();
        assert!(auto_msg.contains("auto"));
        let raw_disk = std::fs::read_to_string(&paths.config_file).unwrap();
        assert!(!raw_disk.contains("\"high\""), "{raw_disk}");
        let stored = AppConfig::load(&paths).unwrap();
        assert_eq!(
            display_thinking_level(&stored.providers[0].thinking_level),
            "auto"
        );

        let mut overridden = stored;
        apply_thinking_override(&mut overridden, Some("high")).unwrap();
        assert_eq!(overridden.providers[0].thinking_level, "high");
        apply_thinking_override(&mut overridden, Some("auto")).unwrap();
        assert!(overridden.providers[0].thinking_level.is_empty());
    }

    /// 【思考等级测试】【错误本地化】验证缺失供应商时返回规范提示而不混搭英文词。
    #[test]
    fn missing_provider_reports_localized_error() {
        let mut config = AppConfig::default();
        config.active_provider = "missing".to_string();
        let err = apply_thinking_override(&mut config, Some("high"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("missing"));
        assert!(!err.contains("未知 provider"));
    }
}
