use super::rtk_probe::{rtk_subcommands, suggest_subcommand};
use serde_json::{json, Value};
use std::collections::BTreeSet;

/// 不参与改写的 shell 元字符：复合命令交给 shell 原样执行。
const SHELL_META_CHARS: &[char] = &['|', '&', ';', '>', '<', '`', '$', '(', ')'];

/// 按配置档位改写命令为 rtk 代理形式。
///
/// 参数:
/// - `command`: 原始命令文本
/// - `mode`: 配置档位（auto / rtk / off）
/// - `denylist`: 用户排除的命令，按原始命令名匹配
///
/// 返回:
/// - 需要改写时返回新命令；否则 None
pub(crate) fn rewrite_command(command: &str, mode: &str, denylist: &[String]) -> Option<String> {
    let mode = mode.trim().to_ascii_lowercase();
    if mode != "rtk" && mode != "auto" {
        return None;
    }
    rewrite_with(command, denylist, rtk_subcommands(), suggest_subcommand)
}

/// 改写命令的纯逻辑部分（探测结果由参数注入，便于测试）。
///
/// rtk 不是通用前缀代理：它只认自己的子命令，`rtk make build` 这类未知子命令会以
/// exit 127 直接失败而不是透传。因此采纳 rtk 的映射建议前，必须确认目标子命令真实存在。
///
/// 参数:
/// - `command`: 原始命令文本
/// - `denylist`: 用户排除的命令
/// - `subcommands`: rtk 的全部合法子命令，用作安全网
/// - `suggest`: 向 rtk 询问映射建议的方法
///
/// 返回:
/// - 需要改写时返回新命令
fn rewrite_with(
    command: &str,
    denylist: &[String],
    subcommands: &BTreeSet<String>,
    suggest: impl Fn(&str) -> Option<String>,
) -> Option<String> {
    if subcommands.is_empty() {
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
    // 3. 用户排除项优先，连询问都不发起
    let command_name = command_base_name(first);
    if denylist.iter().any(|item| item.trim() == command_name) {
        return None;
    }
    // 4. 由 rtk 判定该命令对应哪个子命令；它知道 cat 走 read，也知道交互式子命令不该代理
    let sub = suggest(trimmed)?;
    // 5. 安全网：rtk 偶尔建议并不存在的子命令，执行会失败，这里挡掉
    if !subcommands.contains(&sub) {
        return None;
    }
    // 6. 只采用建议里的子命令名，参数沿用原文——询问时命令已按空白切分，
    //    直接采用建议的完整命令行会丢掉 `git commit -m "a b"` 的引号
    let rest = trimmed
        .strip_prefix(first)
        .map(str::trim_start)
        .unwrap_or_default();
    if rest.is_empty() {
        Some(format!("rtk {sub}"))
    } else {
        Some(format!("rtk {sub} {rest}"))
    }
}

/// 从命令首段取出可比较的命令名。
///
/// 去掉目录前缀与 Windows 可执行后缀，`/usr/bin/git` 与 `git.exe` 都能匹配排除项。
///
/// 参数:
/// - `token`: 命令首段原文
///
/// 返回:
/// - 归一化后的命令名
fn command_base_name(token: &str) -> &str {
    token
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(token)
        .trim_end_matches(".exe")
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

    /// 构造一组 rtk 子命令。
    ///
    /// 参数:
    /// - `names`: 子命令名
    ///
    /// 返回:
    /// - 子命令集合
    fn subcommands(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    /// 模拟 rtk 的映射建议。
    ///
    /// 参数:
    /// - `command`: 原始命令
    ///
    /// 返回:
    /// - 建议的子命令名
    fn fake_suggest(command: &str) -> Option<String> {
        let first = command.split_whitespace().next()?;
        match command_base_name(first) {
            // rtk 把 cat 映射到自己的 read 适配器
            "cat" => Some("read".to_string()),
            // rtk 对不支持的命令也可能给出无效建议
            "make" => Some("make".to_string()),
            // 交互式子命令不建议代理
            "vim" | "sed" => None,
            other => Some(other.to_string()),
        }
    }

    #[test]
    fn rewrites_using_rtk_suggestion() {
        let subs = subcommands(&["git", "cargo", "read", "ls"]);
        assert_eq!(
            rewrite_with("git status", &[], &subs, fake_suggest),
            Some("rtk git status".to_string())
        );
        // 命令名与目标子命令不同：cat 映射到 read
        assert_eq!(
            rewrite_with("cat a.txt", &[], &subs, fake_suggest),
            Some("rtk read a.txt".to_string())
        );
        // 无参数命令不留多余空格
        assert_eq!(
            rewrite_with("ls", &[], &subs, fake_suggest),
            Some("rtk ls".to_string())
        );
    }

    /// rtk 对未知子命令以 exit 127 失败，因此无效建议必须被安全网挡下。
    #[test]
    fn rejects_suggestions_outside_subcommand_set() {
        let subs = subcommands(&["git", "cargo", "read"]);
        assert_eq!(rewrite_with("make build", &[], &subs, fake_suggest), None);
    }

    #[test]
    fn skips_commands_rtk_declines() {
        let subs = subcommands(&["git", "read"]);
        assert_eq!(rewrite_with("vim notes.md", &[], &subs, fake_suggest), None);
        assert_eq!(
            rewrite_with("sed -n 1p a.txt", &[], &subs, fake_suggest),
            None
        );
    }

    /// 询问时命令按空白切分会丢引号，因此参数必须沿用原文。
    #[test]
    fn preserves_original_argument_text() {
        let subs = subcommands(&["git"]);
        assert_eq!(
            rewrite_with(r#"git commit -m "a b""#, &[], &subs, fake_suggest),
            Some(r#"rtk git commit -m "a b""#.to_string())
        );
    }

    #[test]
    fn skips_compound_and_repeated_wrapping() {
        let subs = subcommands(&["git", "cargo"]);
        assert_eq!(
            rewrite_with("git status | head", &[], &subs, fake_suggest),
            None
        );
        assert_eq!(
            rewrite_with("cargo test && echo ok", &[], &subs, fake_suggest),
            None
        );
        assert_eq!(
            rewrite_with("git log > log.txt", &[], &subs, fake_suggest),
            None
        );
        assert_eq!(
            rewrite_with("rtk git status", &[], &subs, fake_suggest),
            None
        );
    }

    #[test]
    fn denylist_takes_priority_over_suggestion() {
        let subs = subcommands(&["git", "ls"]);
        let denylist = vec!["ls".to_string()];
        assert_eq!(rewrite_with("ls -la", &denylist, &subs, fake_suggest), None);
        assert_eq!(
            rewrite_with("git status", &denylist, &subs, fake_suggest),
            Some("rtk git status".to_string())
        );
        // 带路径的命令同样匹配排除项
        assert_eq!(
            rewrite_with("/bin/ls -la", &denylist, &subs, fake_suggest),
            None
        );
    }

    #[test]
    fn unavailable_rtk_disables_rewrite() {
        assert_eq!(
            rewrite_with("git status", &[], &BTreeSet::new(), fake_suggest),
            None
        );
        // off / 未知档位直接关闭，探测都不触发
        assert_eq!(rewrite_command("git status", "off", &[]), None);
        assert_eq!(rewrite_command("git status", "unknown", &[]), None);
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
