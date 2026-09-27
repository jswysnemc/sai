use super::*;
use crate::config::JevConnectionSource;

/// `/jev` 子命令。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JevCommand {
    /// 查看接入与两项功能的状态
    Status,
    /// 测试当前接入
    Test,
    /// 开关工具与 skills 暴露决策
    Routing(bool),
    /// 开关权限自动审核
    Audit(bool),
}

/// 【终端】【Jev命令】处理 `/jev`；非本命令时返回 false 交给后续分发。
///
/// 参数: `input` 为提交文本，其余为存储路径、当前配置、客户端、Agent、界面、权限模式与思考覆盖
/// 返回: 已处理时返回 true；子命令错误以提示行展示，不中断 REPL
#[allow(clippy::too_many_arguments)]
pub(super) async fn handle(
    input: &str,
    paths: &SaiPaths,
    config: &mut AppConfig,
    client: &mut OpenAiCompatibleClient,
    agent: &mut Agent,
    runtime: &mut ReplRuntime,
    mode: AgentMode,
    thinking_override: Option<&str>,
) -> Result<bool> {
    let Some(rest) = repl_command_rest(input, "/jev") else {
        return Ok(false);
    };
    let command = match parse(rest) {
        Ok(command) => command,
        Err(hint) => {
            runtime.record_meta(hint)?;
            return Ok(true);
        }
    };
    match command {
        JevCommand::Status => runtime.record_meta(status_text(config, mode))?,
        JevCommand::Test => {
            // 1. 接入不可用时直接说明原因，不发请求
            let text = match config.jev_connection() {
                Ok(connection) => crate::jev::probe::probe(&connection).await.summary(),
                Err(error) => format!("{error:#}"),
            };
            runtime.record_meta(text)?;
        }
        JevCommand::Routing(enabled) | JevCommand::Audit(enabled) => {
            // 2. 修改磁盘上的原始配置，避免把 Agent 覆盖写回文件
            let mut stored = AppConfig::load(paths)?;
            if matches!(command, JevCommand::Routing(_)) {
                stored.jev.routing.enabled = enabled;
            } else {
                stored.jev.audit.enabled = enabled;
            }
            stored.validate()?;
            stored.save(paths)?;
            // 3. 重建 Agent，使工具网关与审核后端立即按新配置生效
            reload_repl_agent(paths, config, client, agent, mode, thinking_override)?;
            runtime.record_meta(status_text(config, mode))?;
        }
    }
    Ok(true)
}

/// 解析 `/jev` 后的参数。
///
/// 参数:
/// - `rest`: 命令名之后的文本
///
/// 返回:
/// - 子命令；无法识别时返回用法提示
fn parse(rest: &str) -> std::result::Result<JevCommand, String> {
    let words = rest.split_whitespace().map(str::to_ascii_lowercase).collect::<Vec<_>>();
    let words = words.iter().map(String::as_str).collect::<Vec<_>>();
    match words.as_slice() {
        [] | ["status"] => Ok(JevCommand::Status),
        ["test"] => Ok(JevCommand::Test),
        ["routing", switch] => parse_switch(switch).map(JevCommand::Routing),
        ["audit", switch] => parse_switch(switch).map(JevCommand::Audit),
        _ => Err(usage()),
    }
}

/// 解析开关参数。
fn parse_switch(value: &str) -> std::result::Result<bool, String> {
    match value {
        "on" => Ok(true),
        "off" => Ok(false),
        _ => Err(usage()),
    }
}

/// `/jev` 用法提示。
fn usage() -> String {
    t(
        "usage: /jev [status | test | routing on|off | audit on|off]",
        "用法：/jev [status | test | routing on|off | audit on|off]",
    )
    .to_string()
}

/// 生成状态文本：接入、密钥与两项功能。
///
/// 参数:
/// - `config`: 当前生效配置
/// - `mode`: 当前权限模式，用于提示审核是否生效
///
/// 返回:
/// - 多行状态文本
fn status_text(config: &AppConfig, mode: AgentMode) -> String {
    // 1. 接入与密钥
    let connection = match config.jev_endpoint() {
        Ok(endpoint) => {
            let info = crate::config::jev_connection_info_for(endpoint);
            let source = match info.source {
                JevConnectionSource::Endpoint => info.name,
                JevConnectionSource::Official => t("official TypeSafe", "TypeSafe 官方").to_string(),
            };
            let key = match crate::config::jev_connection_for(endpoint) {
                Ok(_) => t("key ready", "密钥可用").to_string(),
                Err(error) => format!("{error:#}"),
            };
            format!("{source} · {} · {} · {key}", info.endpoint, info.model)
        }
        Err(error) => format!("{error:#}"),
    };
    // 2. 暴露决策
    let routing = match (config.jev.routing.enabled, config.jev_routing_active()) {
        (false, _) => t("off", "关闭").to_string(),
        (true, true) => t("on", "开启").to_string(),
        (true, false) => t("on, taken over by DeepSeek anchored mode", "开启，已由 DeepSeek 锚定模式接管").to_string(),
    };
    // 3. 审核：只在自动审核权限模式下参与
    let audit = match (config.jev.audit.enabled, mode == AgentMode::AutoAudit) {
        (false, _) => t("off", "关闭").to_string(),
        (true, true) => t("on", "开启").to_string(),
        (true, false) => t("on, applies after /auto", "开启，切换 /auto 后生效").to_string(),
    };
    format!(
        "Jev\n  {}: {connection}\n  {}: {routing}\n  {}: {audit}",
        t("connection", "接入"),
        t("tool & skill routing", "工具与 Skills 暴露决策"),
        t("permission audit", "权限自动审核"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_subcommands() {
        assert_eq!(parse(""), Ok(JevCommand::Status));
        assert_eq!(parse("status"), Ok(JevCommand::Status));
        assert_eq!(parse("TEST"), Ok(JevCommand::Test));
        assert_eq!(parse("routing on"), Ok(JevCommand::Routing(true)));
        assert_eq!(parse("audit off"), Ok(JevCommand::Audit(false)));
    }

    #[test]
    fn rejects_unknown_arguments() {
        assert!(parse("routing").is_err());
        assert!(parse("audit maybe").is_err());
        assert!(parse("reload").is_err());
    }

    #[test]
    fn status_mentions_mode_requirement_for_audit() {
        let mut config = AppConfig::default();
        config.jev.audit.enabled = true;
        let text = status_text(&config, AgentMode::Yolo);
        assert!(text.contains("/auto"), "{text}");
    }
}
