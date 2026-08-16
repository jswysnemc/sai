use crate::config::DEFAULT_SSH_PORT;

/// 远端主机密钥的校验结论。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum KnownHostStatus {
    /// 记录存在且指纹一致，可直接连接
    Known,
    /// 尚无记录，需用户确认指纹后写入
    Unknown,
    /// 记录存在但指纹不符，可能遭遇中间人攻击，必须阻断
    Changed { stored_fingerprint: String },
}

/// 一条待比对的远端主机密钥。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HostKey {
    pub hostname: String,
    pub port: u16,
    /// 密钥算法，如 ssh-ed25519
    pub algorithm: String,
    /// OpenSSH known_hosts 中的 base64 密钥体
    pub key_base64: String,
    /// SHA256 指纹，供用户核对
    pub fingerprint: String,
}

impl HostKey {
    /// 返回 known_hosts 中的主机字段。
    ///
    /// 非默认端口按 OpenSSH 约定写作 `[host]:port`。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - known_hosts 首列文本
    pub(crate) fn host_field(&self) -> String {
        if self.port == DEFAULT_SSH_PORT {
            self.hostname.clone()
        } else {
            format!("[{}]:{}", self.hostname, self.port)
        }
    }

    /// 渲染为 known_hosts 中的一行。
    pub(crate) fn to_line(&self) -> String {
        format!(
            "{} {} {}",
            self.host_field(),
            self.algorithm,
            self.key_base64
        )
    }
}

/// 在 known_hosts 文本中比对远端主机密钥。
///
/// 同一主机可登记多种算法的密钥，只有算法相同才具备可比性：
/// 算法不同属于并存记录，不能据此判定密钥变更。
///
/// 参数:
/// - `text`: known_hosts 文件内容
/// - `key`: 远端返回的主机密钥
///
/// 返回:
/// - 校验结论
pub(crate) fn check_known_host(text: &str, key: &HostKey) -> KnownHostStatus {
    let host_field = key.host_field();
    let mut stored: Option<String> = None;

    for line in text.lines() {
        let line = line.trim();
        // 1. 跳过空行、注释与 OpenSSH 的标记行
        if line.is_empty() || line.starts_with('#') || line.starts_with('@') {
            continue;
        }

        let mut parts = line.split_whitespace();
        let (Some(hosts), Some(algorithm), Some(key_base64)) =
            (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };

        // 2. 首列可用逗号登记多个主机，逐个比对
        if !hosts.split(',').any(|entry| entry == host_field) {
            continue;
        }
        if algorithm != key.algorithm {
            continue;
        }
        if key_base64 == key.key_base64 {
            return KnownHostStatus::Known;
        }
        // 3. 同主机同算法但密钥不同，记下已存指纹供告警展示
        stored = Some(key_base64.to_string());
    }

    match stored {
        Some(stored_fingerprint) => KnownHostStatus::Changed { stored_fingerprint },
        None => KnownHostStatus::Unknown,
    }
}

/// 把新的主机密钥追加到 known_hosts 文本。
///
/// 参数:
/// - `text`: 现有 known_hosts 内容
/// - `key`: 待写入的主机密钥
///
/// 返回:
/// - 追加后的完整文本
pub(crate) fn append_known_host(text: &str, key: &HostKey) -> String {
    let mut next = text.to_string();
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    next.push_str(&key.to_line());
    next.push('\n');
    next
}
