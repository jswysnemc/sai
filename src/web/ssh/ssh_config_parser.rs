use crate::config::DEFAULT_SSH_PORT;

/// 从 `~/.ssh/config` 解析出的候选主机。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SshConfigCandidate {
    /// config 中的 Host 别名，同时用作展示名
    pub alias: String,
    pub hostname: String,
    pub port: u16,
    pub username: String,
    pub identity_file: String,
}

/// 解析 `~/.ssh/config` 文本，提取可导入的主机。
///
/// 只取 Host、HostName、User、Port、IdentityFile 五项：其余指令与 sai 的
/// 连接参数无对应关系。含通配符的 Host 段是模式而非具体主机，一律跳过；
/// 缺少 HostName 的段回落为用别名作主机名，与 OpenSSH 行为一致。
///
/// 参数:
/// - `text`: config 文件内容
///
/// 返回:
/// - 候选主机列表，按文件中出现顺序排列
pub(crate) fn parse_ssh_config(text: &str) -> Vec<SshConfigCandidate> {
    let mut candidates = Vec::new();
    let mut current: Option<SshConfigCandidate> = None;

    for line in text.lines() {
        // 1. 去掉注释与首尾空白，空行不影响当前 Host 段
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }

        let Some((keyword, value)) = split_directive(line) else {
            continue;
        };
        let lowered = keyword.to_ascii_lowercase();

        // 2. 遇到新的 Host 段先收敛上一段
        if lowered == "host" {
            if let Some(candidate) = current.take() {
                push_candidate(&mut candidates, candidate);
            }
            // Host 行可写多个别名，取第一个具体别名作为该段主机
            let alias = value.split_whitespace().find(|alias| !is_pattern(alias));
            current = alias.map(|alias| SshConfigCandidate {
                alias: alias.to_string(),
                hostname: String::new(),
                port: DEFAULT_SSH_PORT,
                username: String::new(),
                identity_file: String::new(),
            });
            continue;
        }

        // 3. 其余指令填充当前 Host 段，段外指令忽略
        let Some(candidate) = current.as_mut() else {
            continue;
        };
        match lowered.as_str() {
            "hostname" => candidate.hostname = value.to_string(),
            "user" => candidate.username = value.to_string(),
            "port" => {
                if let Ok(port) = value.parse::<u16>() {
                    if port > 0 {
                        candidate.port = port;
                    }
                }
            }
            "identityfile" => candidate.identity_file = strip_quotes(value).to_string(),
            _ => {}
        }
    }

    if let Some(candidate) = current.take() {
        push_candidate(&mut candidates, candidate);
    }
    candidates
}

/// 收敛一个 Host 段并补齐缺省主机名。
fn push_candidate(candidates: &mut Vec<SshConfigCandidate>, mut candidate: SshConfigCandidate) {
    if candidate.hostname.is_empty() {
        candidate.hostname = candidate.alias.clone();
    }
    candidates.push(candidate);
}

/// 拆分一行指令为关键字与取值。
///
/// OpenSSH 允许 `Key Value` 与 `Key=Value` 两种写法。
///
/// 参数:
/// - `line`: 已去除注释与空白的单行文本
///
/// 返回:
/// - 关键字与取值；取值为空时返回 None
fn split_directive(line: &str) -> Option<(&str, &str)> {
    let (keyword, value) = match line.find(['=', ' ', '\t']) {
        Some(index) => (&line[..index], line[index + 1..].trim_start_matches(['=', ' ', '\t'])),
        None => return None,
    };
    let value = value.trim();
    if keyword.is_empty() || value.is_empty() {
        return None;
    }
    Some((keyword, value))
}

/// 判断 Host 别名是否为通配模式。
fn is_pattern(alias: &str) -> bool {
    alias.contains('*') || alias.contains('?') || alias.starts_with('!')
}

/// 去掉取值两端的引号。
fn strip_quotes(value: &str) -> &str {
    value
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or(value)
}
