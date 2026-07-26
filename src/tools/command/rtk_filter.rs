use serde_json::{json, Value};
use std::sync::OnceLock;

/// rtk 语义压缩收益明确的命令族白名单。
///
/// 读取类命令（ls/cat/grep）不改写：agent 常依赖其精确输出。
const RTK_COMMAND_ALLOWLIST: &[&str] = &[
    "git", "cargo", "go", "npm", "pnpm", "yarn", "pytest", "jest", "tsc", "ruff", "docker",
    "kubectl",
];

/// 不参与改写的 shell 元字符：复合命令交给 shell 原样执行。
const SHELL_META_CHARS: &[char] = &['|', '&', ';', '>', '<', '`', '$', '(', ')'];

/// 按配置档位改写命令为 rtk 代理形式。
///
/// 参数:
/// - `command`: 原始命令文本
/// - `mode`: 配置档位（auto / rtk / off）
///
/// 返回:
/// - 需要改写时返回新命令；否则 None
pub(super) fn rewrite_command(command: &str, mode: &str) -> Option<String> {
    let mode = mode.trim().to_ascii_lowercase();
    if mode == "off" {
        return None;
    }
    if mode != "rtk" && mode != "auto" {
        return None;
    }
    if !rtk_available() {
        return None;
    }
    rewrite_with_availability(command, true)
}

/// 判断命令是否满足改写条件并生成改写结果（探测结果由参数注入，便于测试）。
///
/// 参数:
/// - `command`: 原始命令文本
/// - `available`: rtk 是否可用
///
/// 返回:
/// - 需要改写时返回新命令
fn rewrite_with_availability(command: &str, available: bool) -> Option<String> {
    if !available {
        return None;
    }
    let trimmed = command.trim();
    // 1. 复合命令（管道、逻辑连接、重定向、替换）保持原样，避免语义偏差
    if trimmed.contains(SHELL_META_CHARS) {
        return None;
    }
    // 2. 已是 rtk 调用或空命令不重复包装
    let first = trimmed.split_whitespace().next()?;
    if first == "rtk" {
        return None;
    }
    // 3. 仅白名单命令族改写：rtk 对它们有明确的语义压缩适配
    if !RTK_COMMAND_ALLOWLIST.contains(&first) {
        return None;
    }
    Some(format!("rtk {trimmed}"))
}

/// 探测 rtk 是否可用（进程内缓存一次）。
///
/// 返回:
/// - PATH 中存在可执行 rtk 时返回 true
fn rtk_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    // 测试环境不依赖宿主机是否安装 rtk
    if cfg!(test) {
        return false;
    }
    *AVAILABLE.get_or_init(|| {
        std::process::Command::new("rtk")
            .arg("--version")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    })
}

/// 在前台命令结果 JSON 中标记输出已被 rtk 压缩。
///
/// 模型据此知晓输出经过滤，必要时可用原始命令重跑获取完整输出。
///
/// 参数:
/// - `result`: 前台结果 JSON 文本
///
/// 返回:
/// - 附带 filtered_by 字段的 JSON；解析失败时原样返回
pub(super) fn tag_filtered_result(result: String) -> String {
    let Ok(mut value) = serde_json::from_str::<Value>(&result) else {
        return result;
    };
    if let Some(object) = value.as_object_mut() {
        object.insert("filtered_by".to_string(), json!("rtk"));
        object.insert(
            "filter_note".to_string(),
            json!(crate::i18n::text(
                "output compressed by rtk; rerun without filter for full output",
                "输出已由 rtk 压缩；如需完整输出可用原始命令重跑"
            )),
        );
        return value.to_string();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_allowlisted_simple_commands() {
        assert_eq!(
            rewrite_with_availability("git status", true),
            Some("rtk git status".to_string())
        );
        assert_eq!(
            rewrite_with_availability("  cargo test --lib  ", true),
            Some("rtk cargo test --lib".to_string())
        );
    }

    #[test]
    fn skips_compound_and_unlisted_commands() {
        // 复合命令保持原样
        assert_eq!(rewrite_with_availability("git status | head", true), None);
        assert_eq!(rewrite_with_availability("cargo test && echo ok", true), None);
        assert_eq!(rewrite_with_availability("git log > log.txt", true), None);
        // 非白名单命令不改写
        assert_eq!(rewrite_with_availability("ls -la", true), None);
        assert_eq!(rewrite_with_availability("cat src/main.rs", true), None);
        // 已是 rtk 调用不重复包装
        assert_eq!(rewrite_with_availability("rtk git status", true), None);
    }

    #[test]
    fn unavailable_or_off_mode_disables_rewrite() {
        assert_eq!(rewrite_with_availability("git status", false), None);
        // off / 未知档位直接关闭（探测都不触发）
        assert_eq!(rewrite_command("git status", "off"), None);
        assert_eq!(rewrite_command("git status", "unknown"), None);
    }

    #[test]
    fn tag_appends_filter_fields_to_json_result() {
        let tagged = tag_filtered_result(r#"{"success":true,"stdout":"ok"}"#.to_string());
        let value: serde_json::Value = serde_json::from_str(&tagged).unwrap();
        assert_eq!(value["filtered_by"], "rtk");
        assert!(value["filter_note"].as_str().is_some());
        // 非 JSON 输出原样返回
        assert_eq!(tag_filtered_result("plain".to_string()), "plain");
    }
}
