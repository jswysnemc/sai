use anyhow::{Context, Result};
use std::net::{IpAddr, SocketAddr};

/// 【网络授权】【目标校验】只接受精确主机名或显式子域通配符，禁止 URL、凭据和端口混入规则。
/// 参数：配置项；返回标准化规则或配置错误。
pub(super) fn rules(domains: &[String]) -> Result<Vec<String>> {
    domains
        .iter()
        .map(|entry| {
            let value = entry.trim().to_ascii_lowercase();
            let domain = value.strip_prefix("*.").unwrap_or(&value);
            anyhow::ensure!(
                !domain.is_empty()
                    && domain.len() <= 253
                    && !domain.ends_with('.')
                    && domain.split('.').all(|part| !part.is_empty()
                        && part
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')),
                "invalid sandbox allowed domain: {entry}"
            );
            Ok(value)
        })
        .collect()
}

/// 【网络授权】【域名匹配】只匹配完整域名边界；参数为主机名和规则，返回是否授权。
pub(super) fn allowed(host: &str, rules: &[String]) -> bool {
    let host = host.to_ascii_lowercase();
    rules.iter().any(|rule| {
        if let Some(suffix) = rule.strip_prefix("*.") {
            host.len() > suffix.len() + 1 && host.ends_with(&format!(".{suffix}"))
        } else {
            host == *rule
        }
    })
}

/// 【网络授权】【公网地址】拒绝回环、私网、链路本地、多播和保留地址，避免 DNS 重绑定访问主机服务。
/// 参数：解析地址；返回是否可作为公网出口。
pub(super) fn public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_documentation()
                || a == 0
                || a >= 224
                || (a == 100 && (64..=127).contains(&b))
                || (a == 198 && (b == 18 || b == 19))
                || (a == 192 && b == 0 && c == 0))
        }
        IpAddr::V6(ip) => ip.to_ipv4_mapped().map_or_else(
            || {
                ip.segments()[0] & 0xe000 == 0x2000
                    && !(ip.segments()[0] == 0x2001 && ip.segments()[1] <= 0x01ff)
                    && !(ip.segments()[0] == 0x2001 && ip.segments()[1] == 0x0db8)
            },
            |ip| public(IpAddr::V4(ip)),
        ),
    }
}

/// 【网络授权】【固定解析】校验域名和标准端口后解析一次，只返回已验证地址供实际连接。
/// 参数：主机、端口和规则；返回固定公网地址列表，拒绝不重新交给系统解析。
pub(super) async fn resolve(host: &str, port: u16, domains: &[String]) -> Result<Vec<SocketAddr>> {
    anyhow::ensure!(matches!(port, 80 | 443), "network port is not allowed");
    anyhow::ensure!(
        allowed(host, domains),
        "network domain is not allowed: {host}"
    );
    let addresses = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio::net::lookup_host((host, port)),
    )
    .await
    .context("network domain resolution timed out")?
    .context("resolve approved network domain")?
    .collect::<Vec<_>>();
    anyhow::ensure!(
        !addresses.is_empty() && addresses.iter().all(|address| public(address.ip())),
        "network domain resolved to a non-public address"
    );
    Ok(addresses)
}

#[cfg(test)]
mod tests {
    use super::*;
    /// 【网络授权】【匹配回归】禁止后缀伪装、URL规则、私网和映射回环；无参数，返回断言结果。
    #[test]
    fn domain_boundaries_and_private_addresses_are_enforced() {
        let domains = rules(&["example.com".into(), "*.rust-lang.org".into()]).unwrap();
        assert!(allowed("EXAMPLE.COM", &domains));
        assert!(allowed("static.rust-lang.org", &domains));
        assert!(!allowed("example.com.attacker.test", &domains));
        assert!(!allowed("evilrust-lang.org", &domains));
        assert!(!allowed("rust-lang.org", &domains));
        assert!(rules(&["https://example.com".into()]).is_err());
        for address in [
            "127.0.0.1",
            "10.0.0.1",
            "169.254.169.254",
            "100.64.0.1",
            "::1",
            "::ffff:127.0.0.1",
            "fc00::1",
        ] {
            assert!(!public(address.parse().unwrap()));
        }
        assert!(public("1.1.1.1".parse().unwrap()));
    }
}
