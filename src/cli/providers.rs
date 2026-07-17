use super::*;

pub(super) fn run_providers(paths: &SaiPaths, args: ProvidersArgs) -> Result<()> {
    let mut config = AppConfig::load(paths)?;
    let choices = config.provider_model_choices();
    if choices.is_empty() {
        bail!(
            "{}",
            t(
                "no active provider models; configure or activate a model first",
                "没有已激活的 provider 模型；请先配置或激活模型",
            )
        );
    }
    if let Some(index) = args.index {
        if index == 0 || index > choices.len() {
            bail!(
                "{}: {index}",
                t("provider index out of range", "provider 序号超出范围")
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
            t("active provider", "当前 provider")
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
                t("active provider", "当前 provider"),
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

/// 执行配置设置子命令。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `args`: set 子命令参数
///
/// 返回:
/// - 设置是否成功
pub(super) fn run_set(paths: &SaiPaths, args: SetArgs) -> Result<()> {
    match args.command {
        SetCommand::Thinking(args) => run_set_thinking(paths, args),
    }
}

/// 设置当前 provider 的思考等级。
///
/// 参数:
/// - `paths`: Sai 路径
/// - `args`: thinking 参数
///
/// 返回:
/// - 设置是否成功
pub(super) fn run_set_thinking(paths: &SaiPaths, args: SetThinkingArgs) -> Result<()> {
    AppConfig::init_files(paths)?;
    let mut config = AppConfig::load(paths)?;
    let active = config.active_provider.clone();
    let provider = config
        .providers
        .iter_mut()
        .find(|provider| provider.id == active)
        .ok_or_else(|| anyhow::anyhow!("provider not found: {active}"))?;
    let current_level = display_thinking_level(&provider.thinking_level);
    let provider_name = provider.display_name.clone();
    let level = resolve_thinking_level_arg(args.level.as_deref(), &current_level, &provider_name)?;
    provider.thinking_level = level.to_string();
    config.save(paths)?;
    println!(
        "{}: {} / {}",
        t("thinking level", "思考等级"),
        provider_name,
        level
    );
    Ok(())
}

/// 解析或交互选择思考等级参数。
///
/// 参数:
/// - `level`: 命令行传入的思考等级
///
/// 返回:
/// - 有效思考等级
fn resolve_thinking_level_arg(
    level: Option<&str>,
    current_level: &str,
    provider_name: &str,
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
    println!(
        "{}: {} / {}",
        t("current thinking level", "当前思考等级"),
        provider_name,
        current_level
    );
    let labels = THINKING_LEVELS
        .iter()
        .map(|level| {
            if *level == current_level {
                format!("{level} ({})", t("current", "当前"))
            } else {
                level.to_string()
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

/// 校验并归一化思考等级。
///
/// 参数:
/// - `level`: 原始思考等级
///
/// 返回:
/// - 归一化后的思考等级
pub(super) fn normalize_thinking_level(level: &str) -> Result<&'static str> {
    THINKING_LEVELS
        .iter()
        .copied()
        .find(|item| *item == level)
        .ok_or_else(|| anyhow::anyhow!("invalid thinking level: {level}"))
}

/// 返回用于展示的思考等级。
///
/// 参数:
/// - `level`: 配置中的原始思考等级
///
/// 返回:
/// - 非空思考等级，空值显示为 auto
pub(super) fn display_thinking_level(level: &str) -> String {
    let level = level.trim();
    if level.is_empty() {
        "auto".to_string()
    } else {
        level.to_string()
    }
}

/// 对当前激活 provider 应用命令行临时思考等级覆盖。
///
/// 参数:
/// - `config`: 当前应用配置
/// - `level`: 命令行传入的思考等级
///
/// 返回:
/// - 等级非法或 provider 不存在时返回错误
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
        .ok_or_else(|| anyhow::anyhow!("provider not found: {active}"))?;
    provider.thinking_level = level.to_string();
    Ok(())
}
