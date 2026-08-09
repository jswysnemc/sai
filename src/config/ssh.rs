use serde::{Deserialize, Serialize};

/// SSH 默认端口。
pub const DEFAULT_SSH_PORT: u16 = 22;

/// 单个 SSH 主机的连接配置。
///
/// 只保存私钥文件路径，不保存密码与私钥内容：sai 配置以明文落盘，
/// 存放凭据会把风险扩散到配置备份与同步链路。需要口令的私钥在连接时临时索取。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SshHostConfig {
    /// 主机标识，创建后不变，终端会话按此引用
    pub id: String,
    /// 展示名称
    pub label: String,
    /// 主机名或 IP
    pub hostname: String,
    #[serde(default = "default_ssh_port")]
    pub port: u16,
    pub username: String,
    /// 私钥文件路径，留空表示依次尝试默认私钥
    #[serde(default)]
    pub identity_file: String,
    /// 登录后进入的目录，留空表示使用远端默认目录
    #[serde(default)]
    pub remote_directory: String,
}

impl SshHostConfig {
    /// 返回用于展示的主机地址。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - `user@host` 或 `user@host:port` 形式的地址
    pub fn display_address(&self) -> String {
        if self.port == DEFAULT_SSH_PORT {
            format!("{}@{}", self.username, self.hostname)
        } else {
            format!("{}@{}:{}", self.username, self.hostname, self.port)
        }
    }
}

/// SSH 主机列表配置。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SshConfig {
    #[serde(default)]
    pub hosts: Vec<SshHostConfig>,
}

impl SshConfig {
    /// 按标识查找主机。
    ///
    /// 参数:
    /// - `id`: 主机标识
    ///
    /// 返回:
    /// - 命中的主机配置
    pub fn find(&self, id: &str) -> Option<&SshHostConfig> {
        self.hosts.iter().find(|host| host.id == id)
    }
}

/// 返回 SSH 默认端口，供 serde 缺省值使用。
fn default_ssh_port() -> u16 {
    DEFAULT_SSH_PORT
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造测试主机配置。
    fn host(port: u16) -> SshHostConfig {
        SshHostConfig {
            id: "h1".to_string(),
            label: "build box".to_string(),
            hostname: "example.com".to_string(),
            port,
            username: "deploy".to_string(),
            identity_file: String::new(),
            remote_directory: String::new(),
        }
    }

    #[test]
    fn display_address_omits_the_default_port() {
        assert_eq!(host(DEFAULT_SSH_PORT).display_address(), "deploy@example.com");
    }

    #[test]
    fn display_address_keeps_a_custom_port() {
        assert_eq!(host(2222).display_address(), "deploy@example.com:2222");
    }

    #[test]
    fn missing_port_falls_back_to_the_default() {
        let parsed: SshHostConfig = serde_json::from_str(
            r#"{
                "id": "h1",
                "label": "box",
                "hostname": "example.com",
                "username": "deploy"
            }"#,
        )
        .expect("应能解析缺省端口的主机配置");
        assert_eq!(parsed.port, DEFAULT_SSH_PORT);
        assert_eq!(parsed.identity_file, "");
    }

    #[test]
    fn find_returns_the_matching_host() {
        let config = SshConfig {
            hosts: vec![host(DEFAULT_SSH_PORT)],
        };
        assert!(config.find("h1").is_some());
        assert!(config.find("missing").is_none());
    }
}
