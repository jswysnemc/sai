use std::collections::HashSet;
use std::ffi::OsString;

/// 变量名按 `_` 切分后命中任一段即视为密钥。
const SECRET_SEGMENTS: &[&str] = &[
    "TOKEN",
    "SECRET",
    "PASSWORD",
    "PASSWD",
    "CREDENTIAL",
    "CREDENTIALS",
    "APIKEY",
];

/// 以连续片段出现时视为密钥的组合，例如 `OPENAI_API_KEY`。
const SECRET_PAIRS: &[(&str, &str)] = &[("API", "KEY"), ("PRIVATE", "KEY"), ("ACCESS", "KEY")];

/// 无法从名称结构推断、但必须移除的变量。
const SECRET_NAMES: &[&str] = &["AWS_ACCESS_KEY_ID", "AWS_SESSION_TOKEN", "SAI_WEB_PASSWORD"];

/// 判断环境变量名是否像密钥。
///
/// 参数:
/// - `name`: 环境变量名
///
/// 返回:
/// - 命中密钥规则时返回 `true`
pub(crate) fn is_secret_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    if SECRET_NAMES.contains(&upper.as_str()) {
        return true;
    }
    let segments = upper.split(['_', '-']).collect::<Vec<_>>();
    if segments
        .iter()
        .any(|segment| SECRET_SEGMENTS.contains(segment))
    {
        return true;
    }
    segments
        .windows(2)
        .any(|pair| SECRET_PAIRS.contains(&(pair[0], pair[1])))
}

/// 从环境变量集合中移除密钥类变量。
///
/// 参数:
/// - `vars`: 原始环境变量
/// - `scrub`: 是否启用清理
/// - `passthrough`: 清理时仍保留的变量名
///
/// 返回:
/// - 保留的变量与被移除的变量名
pub(crate) fn scrub_env(
    vars: impl IntoIterator<Item = (OsString, OsString)>,
    scrub: bool,
    passthrough: &[String],
) -> (Vec<(OsString, OsString)>, Vec<String>) {
    let keep = passthrough
        .iter()
        .map(|name| name.trim().to_ascii_uppercase())
        .collect::<HashSet<_>>();
    let mut kept = Vec::new();
    let mut removed = Vec::new();
    for (key, value) in vars {
        let name = key.to_string_lossy();
        if scrub && is_secret_name(&name) && !keep.contains(&name.to_ascii_uppercase()) {
            removed.push(name.into_owned());
            continue;
        }
        kept.push((key, value));
    }
    removed.sort();
    (kept, removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证常见供应商密钥被识别，普通变量不受影响。
    #[test]
    fn detects_secret_names_without_false_positives() {
        for name in [
            "OPENAI_API_KEY",
            "ANTHROPIC_API_KEY",
            "GITHUB_TOKEN",
            "GH_TOKEN",
            "AWS_SECRET_ACCESS_KEY",
            "AWS_ACCESS_KEY_ID",
            "DB_PASSWORD",
            "NPM_TOKEN",
            "DEEPSEEK_APIKEY",
        ] {
            assert!(is_secret_name(name), "{name}");
        }
        for name in [
            "PATH",
            "HOME",
            "SSH_AUTH_SOCK",
            "GIT_AUTHOR_NAME",
            "TOKENIZERS_PARALLELISM",
            "KEYTIMEOUT",
            "XAUTHORITY",
        ] {
            assert!(!is_secret_name(name), "{name}");
        }
    }

    /// 验证白名单变量保留、关闭清理时全部保留。
    #[test]
    fn scrub_respects_passthrough_and_switch() {
        let vars = vec![
            (OsString::from("PATH"), OsString::from("/bin")),
            (OsString::from("GITHUB_TOKEN"), OsString::from("x")),
            (OsString::from("OPENAI_API_KEY"), OsString::from("y")),
        ];
        let (kept, removed) = scrub_env(vars.clone(), true, &["github_token".into()]);
        assert_eq!(kept.len(), 2);
        assert_eq!(removed, vec!["OPENAI_API_KEY".to_string()]);
        let (kept, removed) = scrub_env(vars, false, &[]);
        assert_eq!(kept.len(), 3);
        assert!(removed.is_empty());
    }
}
