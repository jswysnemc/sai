use std::collections::BTreeSet;
use std::process::Command;
use std::sync::OnceLock;

/// rtk 自有功能命令，不作为「用户敲的命令」被自动代理。
///
/// rtk 的子命令分两类：一类是同名系统命令的代理（`rtk git` 之于 `git`），
/// 另一类是 rtk 自己的功能入口。后者与同名系统命令语义不同，误代理会改变行为——
/// 最典型的是 `test`：shell 的 `test -f x` 是条件判断，`rtk test` 是「跑测试只看失败」。
///
/// 注意这只约束「用户输入的命令名」。rtk 自己给出的映射建议不受此限：
/// `cat a.txt` 被 rtk 映射为 `rtk read a.txt` 是正确的，read 作为**目标**合法。
const RTK_OWN_COMMANDS: &[&str] = &[
    // 管理与统计入口
    "init",
    "config",
    "gain",
    "cc-economics",
    "discover",
    "session",
    "telemetry",
    "learn",
    "trust",
    "untrust",
    "verify",
    "hook",
    "hook-audit",
    "rewrite",
    // 执行器：套在别的命令外面，不能再被自动套一层
    "run",
    "proxy",
    "pipe",
    // 与 shell 内建或常见脚本名冲突
    "test",
    "read",
    "smart",
    "summary",
    "err",
    "deps",
    "json",
    "log",
    "lint",
    "format",
];

/// 返回 rtk 的全部合法子命令（进程内探测一次）。
///
/// 用作安全网：rtk 的 `rewrite` 偶尔会给出并不存在的子命令建议
/// （例如把 `make build` 建议成 `rtk make build`，而 rtk 并没有 make 适配器），
/// 直接执行会以 exit 127 失败。采纳建议前必须确认目标子命令真实存在。
///
/// 返回:
/// - 子命令名集合；rtk 不可用时为空集
pub(crate) fn rtk_subcommands() -> &'static BTreeSet<String> {
    static SUBCOMMANDS: OnceLock<BTreeSet<String>> = OnceLock::new();
    SUBCOMMANDS.get_or_init(|| {
        // 测试环境不依赖宿主机是否安装 rtk
        if cfg!(test) {
            return BTreeSet::new();
        }
        let Ok(output) = Command::new("rtk").arg("--help").output() else {
            return BTreeSet::new();
        };
        if !output.status.success() {
            return BTreeSet::new();
        }
        parse_subcommands(&String::from_utf8_lossy(&output.stdout))
    })
}

/// 返回可作为用户命令入口被自动代理的命令集合。
///
/// 即全部子命令扣除 rtk 自有功能命令，供设置界面展示「rtk 现在能代理什么」。
///
/// 返回:
/// - 可代理命令名集合
pub(crate) fn rtk_proxy_commands() -> &'static BTreeSet<String> {
    static PROXYABLE: OnceLock<BTreeSet<String>> = OnceLock::new();
    PROXYABLE.get_or_init(|| {
        rtk_subcommands()
            .iter()
            .filter(|name| !RTK_OWN_COMMANDS.contains(&name.as_str()))
            .cloned()
            .collect()
    })
}

/// 探测 rtk 是否可用。
///
/// 返回:
/// - PATH 中存在可执行 rtk 且能解析出子命令时返回 true
pub(crate) fn rtk_available() -> bool {
    !rtk_subcommands().is_empty()
}

/// 从 `rtk --help` 输出解析子命令集合。
///
/// 参数:
/// - `help`: `rtk --help` 的标准输出
///
/// 返回:
/// - 子命令名集合
pub(super) fn parse_subcommands(help: &str) -> BTreeSet<String> {
    let mut commands = BTreeSet::new();
    let mut in_commands = false;
    for line in help.lines() {
        // 1. 定位 Commands 段；下一个顶格段落标题即为结束
        if line.trim_start().starts_with("Commands:") {
            in_commands = true;
            continue;
        }
        if !in_commands {
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        if !line.starts_with(char::is_whitespace) {
            break;
        }
        // 2. 每行形如 `  name    description`，取首个词作为子命令名
        let Some(name) = line.split_whitespace().next() else {
            continue;
        };
        if name == "help" {
            continue;
        }
        if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            commands.insert(name.to_string());
        }
    }
    commands
}

/// 向 rtk 询问某条命令对应的代理子命令。
///
/// `rtk rewrite` 是 rtk 自己的映射入口，比按命令名硬匹配准确得多：
/// 它知道 `cat` 应该走 `rtk read`，也知道 `git config --edit` 这类交互式子命令
/// 不该被代理（返回空）。
///
/// 只取回建议里的子命令名，不采用它输出的完整命令行——
/// 传参时命令已按空白切分，引号信息丢失，直接采用会改变 `git commit -m "a b"` 的语义。
///
/// 参数:
/// - `command`: 原始命令文本
///
/// 返回:
/// - rtk 建议的子命令名；不建议代理时返回 None
pub(super) fn suggest_subcommand(command: &str) -> Option<String> {
    let output = Command::new("rtk")
        .arg("rewrite")
        .args(command.split_whitespace())
        .output()
        .ok()?;
    parse_suggestion(&String::from_utf8_lossy(&output.stdout))
}

/// 从 `rtk rewrite` 的输出提取建议的子命令名。
///
/// 参数:
/// - `stdout`: `rtk rewrite` 的标准输出
///
/// 返回:
/// - 形如 `rtk <sub> ...` 时返回 `sub`；其余情况返回 None
pub(super) fn parse_suggestion(stdout: &str) -> Option<String> {
    let mut tokens = stdout.split_whitespace();
    if tokens.next()? != "rtk" {
        return None;
    }
    let sub = tokens.next()?;
    if sub.is_empty() || sub.starts_with('-') {
        return None;
    }
    Some(sub.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 与本机 rtk 0.43 一致的 help 片段。
    const HELP_SAMPLE: &str = concat!(
        "A high-performance CLI proxy.\n\n",
        "Usage: rtk [OPTIONS] <COMMAND>\n\n",
        "Commands:\n",
        "  ls             List directory contents\n",
        "  git            Git commands with compact output\n",
        "  read           Read file with intelligent filtering\n",
        "  test           Run tests and show only failures\n",
        "  golangci-lint  Go linter with compact output\n",
        "  help           Print this message\n\n",
        "Options:\n",
        "  -h, --help     Print help\n",
    );

    #[test]
    fn parses_subcommands_including_rtk_own_entries() {
        let parsed = parse_subcommands(HELP_SAMPLE);
        assert!(parsed.contains("ls"));
        assert!(parsed.contains("git"));
        // 自有命令仍属于合法子命令，安全网需要认得它们
        assert!(parsed.contains("read"));
        assert!(parsed.contains("test"));
        // 带连字符的命令名保留
        assert!(parsed.contains("golangci-lint"));
        // help 与 Options 段不参与
        assert!(!parsed.contains("help"));
        assert!(!parsed.iter().any(|name| name.starts_with('-')));
    }

    #[test]
    fn extracts_subcommand_from_suggestion() {
        assert_eq!(parse_suggestion("rtk git status"), Some("git".to_string()));
        // rtk 会把 cat 映射到 read，映射目标与原命令名不同
        assert_eq!(parse_suggestion("rtk read a.txt"), Some("read".to_string()));
        assert_eq!(parse_suggestion("  rtk ls -la  \n"), Some("ls".to_string()));
    }

    #[test]
    fn rejects_non_suggestions() {
        // 不建议代理时输出为空
        assert_eq!(parse_suggestion(""), None);
        assert_eq!(parse_suggestion("   \n"), None);
        // 输出不以 rtk 开头时不采纳
        assert_eq!(parse_suggestion("git status"), None);
        // 只有 rtk 没有子命令
        assert_eq!(parse_suggestion("rtk"), None);
        assert_eq!(parse_suggestion("rtk --help"), None);
    }
}
