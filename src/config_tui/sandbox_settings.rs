//! 全局参数中的沙箱小节：开关、网络策略、环境变量清理与路径列表。

use crate::config::{AppConfig, SandboxConfig, SandboxNetworkMode};
use crate::i18n::text as t;
use anyhow::Result;
use std::io;

use super::form::{parse_bool_field, run_form, Field};
use super::ui::message;

/// 生成菜单右侧的沙箱摘要。
///
/// 参数:
/// - `config`: 应用配置
///
/// 返回:
/// - 摘要文本
pub(super) fn sandbox_details(config: &AppConfig) -> String {
    let sandbox = &config.sandbox;
    format!(
        "{}\n\n{}: {} · {}: {} · {}: {}",
        t(
            "OS isolation for run_command in audited and plan modes: bubblewrap on Linux, Seatbelt on macOS.",
            "审核与计划模式下 run_command 的系统级隔离：Linux 使用 bubblewrap，macOS 使用 Seatbelt。",
        ),
        t("Sandbox", "沙箱"),
        if sandbox.enabled { t("on", "启用") } else { t("off", "关闭") },
        t("Network", "网络"),
        sandbox.network.as_str(),
        t("Scrub env", "清理密钥变量"),
        if sandbox.scrub_env { t("on", "启用") } else { t("off", "关闭") },
    )
}

/// 【配置界面】【沙箱】编辑沙箱配置；任一字段无效时不写入。
///
/// 参数:
/// - `stdout`: 终端标准输出
/// - `config`: 待更新应用配置
///
/// 返回:
/// - 编辑结果
pub(super) fn edit_sandbox_settings(stdout: &mut io::Stdout, config: &mut AppConfig) -> Result<()> {
    let sandbox = &config.sandbox;
    let mut fields = vec![
        Field::boolean(t("Command sandbox", "命令沙箱"), sandbox.enabled),
        Field::new(
            t("Network in sandbox", "沙箱内网络"),
            sandbox.network.as_str().to_string(),
        )
        .choices(&["deny", "allow"]),
        Field::boolean(
            t("Scrub secret env vars", "清理密钥环境变量"),
            sandbox.scrub_env,
        ),
        Field::new(
            t(
                "Extra writable roots (comma separated)",
                "额外可写目录（逗号分隔）",
            ),
            sandbox.writable_roots.join(", "),
        ),
        Field::new(
            t(
                "Extra hidden paths (comma separated)",
                "额外隐藏路径（逗号分隔）",
            ),
            sandbox.deny_read.join(", "),
        ),
        Field::new(
            t(
                "Env vars kept when scrubbing (comma separated)",
                "清理时保留的变量（逗号分隔）",
            ),
            sandbox.env_passthrough.join(", "),
        ),
    ];
    loop {
        if !run_form(stdout, t(" SANDBOX ", " 沙箱 "), &mut fields)? {
            return Ok(());
        }
        match parse_fields(&fields) {
            Ok(next) => {
                config.sandbox = next;
                return Ok(());
            }
            Err(error) => message(
                stdout,
                &format!("{}: {error}", t("Invalid input", "输入无效")),
            )?,
        }
    }
}

/// 把表单字段解析为沙箱配置。
///
/// 参数:
/// - `fields`: 表单字段，顺序与 `edit_sandbox_settings` 一致
///
/// 返回:
/// - 沙箱配置
fn parse_fields(fields: &[Field]) -> Result<SandboxConfig> {
    Ok(SandboxConfig {
        enabled: parse_bool_field(&fields[0].value)?,
        network: SandboxNetworkMode::parse_or_default(&fields[1].value),
        scrub_env: parse_bool_field(&fields[2].value)?,
        writable_roots: split_list(&fields[3].value),
        deny_read: split_list(&fields[4].value),
        env_passthrough: split_list(&fields[5].value),
    })
}

/// 按逗号拆分列表并去掉空项。
fn split_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证表单字段解析为配置，列表去除空项。
    #[test]
    fn parses_form_fields() {
        let fields = vec![
            Field::boolean("a", false),
            Field::new("b", "allow".into()),
            Field::boolean("c", true),
            Field::new("d", "~/.cargo, ,/opt/cache".into()),
            Field::new("e", ".env".into()),
            Field::new("f", String::new()),
        ];
        let config = parse_fields(&fields).unwrap();
        assert!(!config.enabled);
        assert_eq!(config.network, SandboxNetworkMode::Allow);
        assert!(config.scrub_env);
        assert_eq!(config.writable_roots, vec!["~/.cargo", "/opt/cache"]);
        assert_eq!(config.deny_read, vec![".env"]);
        assert!(config.env_passthrough.is_empty());
    }
}
