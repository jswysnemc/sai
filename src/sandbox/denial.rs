use crate::i18n::text as t;
use serde_json::{json, Value};

/// 沙箱拦截类型。
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum DenialKind {
    /// 写入只读路径或读取隐藏路径
    Filesystem,
    /// 断网后的连接或解析失败
    Network,
}

impl DenialKind {
    /// 返回结果 JSON 使用的稳定字符串。
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Filesystem => "filesystem",
            Self::Network => "network",
        }
    }
}

/// 文件系统拦截的典型报错；命令继承用户语言环境，需同时覆盖本地化文本。
const FILESYSTEM_MARKERS: &[&str] = &[
    "read-only file system",
    "operation not permitted",
    "sandbox-exec: ",
    "deny file-write",
    "只读文件系统",
    "不允许的操作",
    "唯讀檔案系統",
];

/// 断网后的典型报错。
const NETWORK_MARKERS: &[&str] = &[
    "could not resolve host",
    "temporary failure in name resolution",
    "name or service not known",
    "network is unreachable",
    "nodename nor servname provided",
    "failed to lookup address information",
    "getaddrinfo",
    "no address associated with hostname",
    "connection refused",
    "无法解析主机",
    "域名解析暂时失败",
    "网络不可达",
];

/// 【沙箱】【拦截识别】根据失败命令的输出判断是否由沙箱拦截。
///
/// 参数:
/// - `stdout`: 标准输出
/// - `stderr`: 标准错误
/// - `network_allowed`: 沙箱是否保留网络；保留时不把网络错误归因于沙箱
///
/// 返回:
/// - 拦截类型与命中的输出行
pub(crate) fn detect_denial(
    stdout: &str,
    stderr: &str,
    network_allowed: bool,
) -> Option<(DenialKind, String)> {
    for line in stderr.lines().chain(stdout.lines()) {
        let lower = line.to_ascii_lowercase();
        if FILESYSTEM_MARKERS
            .iter()
            .any(|marker| lower.contains(marker))
        {
            return Some((DenialKind::Filesystem, clip(line)));
        }
        if !network_allowed && NETWORK_MARKERS.iter().any(|marker| lower.contains(marker)) {
            return Some((DenialKind::Network, clip(line)));
        }
    }
    None
}

/// 生成写入命令结果的拦截说明，指引模型申请提升权限而不是反复重试。
///
/// 参数:
/// - `kind`: 拦截类型
/// - `evidence`: 命中的输出行
/// - `read_only`: 是否为计划模式只读沙箱
///
/// 返回:
/// - `sandbox_denial` 字段内容
pub(crate) fn denial_report(kind: DenialKind, evidence: &str, read_only: bool) -> Value {
    let hint = if read_only {
        t(
            "Plan mode runs commands in a read-only sandbox. Do not retry writes; describe the change in the plan instead.",
            "计划模式在只读沙箱中执行命令。不要重试写入，请把改动写进计划。",
        )
    } else {
        match kind {
            DenialKind::Filesystem => t(
                "The sandbox blocked a write outside the workspace or a hidden path. If the command really needs it, rerun with sandbox_permissions=\"require_escalated\" and a justification.",
                "沙箱拦截了工作区外写入或隐藏路径访问。确有需要时，使用 sandbox_permissions=\"require_escalated\" 并填写 justification 重新执行。",
            ),
            DenialKind::Network => t(
                "The sandbox has no network access. If the command needs the network, rerun with sandbox_permissions=\"require_escalated\" and a justification.",
                "沙箱内没有网络。命令需要联网时，使用 sandbox_permissions=\"require_escalated\" 并填写 justification 重新执行。",
            ),
        }
    };
    json!({
        "kind": kind.as_str(),
        "evidence": evidence,
        "hint": hint,
    })
}

/// 截断过长的证据行。
fn clip(line: &str) -> String {
    let line = line.trim();
    match line.char_indices().nth(240) {
        Some((index, _)) => format!("{}…", &line[..index]),
        None => line.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证只读文件系统与断网报错分别被识别。
    #[test]
    fn detects_filesystem_and_network_denials() {
        let (kind, evidence) = detect_denial(
            "",
            "touch: cannot touch '../x': Read-only file system",
            false,
        )
        .unwrap();
        assert_eq!(kind, DenialKind::Filesystem);
        assert!(evidence.contains("Read-only"));
        let (kind, _) =
            detect_denial("", "curl: (6) Could not resolve host: example.com", false).unwrap();
        assert_eq!(kind, DenialKind::Network);
    }

    /// 验证中文语言环境下的报错同样被识别。
    #[test]
    fn detects_localized_messages() {
        let (kind, _) =
            detect_denial("", "touch: 无法 touch '../blocked': 只读文件系统", false).unwrap();
        assert_eq!(kind, DenialKind::Filesystem);
        let (kind, _) = detect_denial("", "curl: (6) 无法解析主机：example.com", false).unwrap();
        assert_eq!(kind, DenialKind::Network);
    }

    /// 验证允许网络时不把网络错误归因于沙箱，普通失败不误判。
    #[test]
    fn ignores_unrelated_failures() {
        assert!(detect_denial("", "Could not resolve host: x", true).is_none());
        assert!(detect_denial("test failed", "error[E0308]: mismatched types", false).is_none());
    }

    /// 验证说明文本区分计划模式与提升提示。
    #[test]
    fn report_points_to_escalation() {
        let report = denial_report(DenialKind::Network, "x", false);
        assert_eq!(report["kind"], "network");
        assert!(report["hint"]
            .as_str()
            .unwrap()
            .contains("require_escalated"));
        let plan = denial_report(DenialKind::Filesystem, "x", true);
        assert!(!plan["hint"].as_str().unwrap().contains("require_escalated"));
    }
}
