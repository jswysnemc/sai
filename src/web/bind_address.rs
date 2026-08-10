use anyhow::{Context, Result};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

/// 解析监听地址。
///
/// 支持 IPv4 与 IPv6 字面量；`localhost` 视作回环地址，省去用户记忆写法。
///
/// 参数:
/// - `host`: 命令行传入的监听地址
/// - `port`: 监听端口
///
/// 返回:
/// - 可用于绑定的套接字地址
pub(super) fn resolve_bind_address(host: &str, port: u16) -> Result<SocketAddr> {
    let host = host.trim();
    if host.eq_ignore_ascii_case("localhost") {
        return Ok(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port));
    }
    let ip: IpAddr = host
        .parse()
        .with_context(|| format!("invalid listen address: {host}"))?;
    Ok(SocketAddr::new(ip, port))
}

/// 判断该地址是否会把服务暴露到本机之外。
///
/// 参数:
/// - `address`: 实际绑定的套接字地址
///
/// 返回:
/// - 非回环地址时返回 true
pub(super) fn is_externally_reachable(address: &SocketAddr) -> bool {
    !address.ip().is_loopback()
}

/// 组装用户可直接打开的访问地址。
///
/// 通配地址（0.0.0.0 与 ::）不是可访问目标，浏览器无法据此连接，
/// 这里回落为回环地址，用户仍能在本机打开；对外访问由用户按实际网卡地址替换。
///
/// 参数:
/// - `address`: 实际绑定的套接字地址
/// - `token`: 本次启动的访问令牌
///
/// 返回:
/// - 带令牌的访问地址
pub(super) fn browsable_url(address: &SocketAddr, token: &str) -> String {
    let port = address.port();
    let host = match address.ip() {
        IpAddr::V4(ip) if ip.is_unspecified() => Ipv4Addr::LOCALHOST.to_string(),
        IpAddr::V6(ip) if ip.is_unspecified() => Ipv4Addr::LOCALHOST.to_string(),
        // IPv6 字面量在 URL 中必须放进方括号
        IpAddr::V6(ip) => format!("[{ip}]"),
        IpAddr::V4(ip) => ip.to_string(),
    };
    format!("http://{host}:{port}/?token={token}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_ipv4_and_wildcard_addresses() {
        assert_eq!(
            resolve_bind_address("127.0.0.1", 4096).unwrap(),
            "127.0.0.1:4096".parse::<SocketAddr>().unwrap()
        );
        assert_eq!(
            resolve_bind_address("0.0.0.0", 4096).unwrap(),
            "0.0.0.0:4096".parse::<SocketAddr>().unwrap()
        );
    }

    #[test]
    fn resolves_localhost_and_ignores_case_and_padding() {
        let expected = "127.0.0.1:4096".parse::<SocketAddr>().unwrap();
        assert_eq!(resolve_bind_address("localhost", 4096).unwrap(), expected);
        assert_eq!(resolve_bind_address("  LocalHost  ", 4096).unwrap(), expected);
    }

    #[test]
    fn resolves_ipv6_addresses() {
        assert_eq!(
            resolve_bind_address("::1", 4096).unwrap(),
            "[::1]:4096".parse::<SocketAddr>().unwrap()
        );
        assert_eq!(
            resolve_bind_address("::", 4096).unwrap(),
            "[::]:4096".parse::<SocketAddr>().unwrap()
        );
    }

    #[test]
    fn rejects_hostnames_and_malformed_input() {
        // 只接受地址字面量：主机名解析结果可能多值且随 DNS 变化，绑定目标必须确定
        assert!(resolve_bind_address("example.com", 4096).is_err());
        assert!(resolve_bind_address("999.1.1.1", 4096).is_err());
        assert!(resolve_bind_address("", 4096).is_err());
    }

    #[test]
    fn marks_non_loopback_addresses_as_externally_reachable() {
        assert!(is_externally_reachable(
            &"0.0.0.0:4096".parse::<SocketAddr>().unwrap()
        ));
        assert!(is_externally_reachable(
            &"192.168.1.10:4096".parse::<SocketAddr>().unwrap()
        ));
        assert!(is_externally_reachable(
            &"[::]:4096".parse::<SocketAddr>().unwrap()
        ));
    }

    #[test]
    fn treats_loopback_addresses_as_local_only() {
        assert!(!is_externally_reachable(
            &"127.0.0.1:4096".parse::<SocketAddr>().unwrap()
        ));
        assert!(!is_externally_reachable(
            &"[::1]:4096".parse::<SocketAddr>().unwrap()
        ));
    }

    #[test]
    fn rewrites_wildcard_addresses_into_a_browsable_url() {
        assert_eq!(
            browsable_url(&"0.0.0.0:4096".parse().unwrap(), "abc"),
            "http://127.0.0.1:4096/?token=abc"
        );
        assert_eq!(
            browsable_url(&"[::]:4096".parse().unwrap(), "abc"),
            "http://127.0.0.1:4096/?token=abc"
        );
    }

    #[test]
    fn keeps_concrete_addresses_and_brackets_ipv6() {
        assert_eq!(
            browsable_url(&"192.168.1.10:4096".parse().unwrap(), "abc"),
            "http://192.168.1.10:4096/?token=abc"
        );
        assert_eq!(
            browsable_url(&"[::1]:4096".parse().unwrap(), "abc"),
            "http://[::1]:4096/?token=abc"
        );
    }
}
