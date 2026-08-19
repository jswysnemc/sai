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
    format!("{}/?token={token}", browsable_origin(address))
}

/// 组装不带启动令牌的访问地址。
///
/// 参数:
/// - `address`: 实际绑定的套接字地址
///
/// 返回:
/// - 不含令牌的本机访问地址
pub(super) fn browsable_url_without_token(address: &SocketAddr) -> String {
    browsable_origin(address)
}

/// 组装浏览器可打开的主机与端口。
///
/// 参数:
/// - `address`: 实际绑定的套接字地址
///
/// 返回:
/// - `http://host:port`
fn browsable_origin(address: &SocketAddr) -> String {
    let port = address.port();
    let host = match address.ip() {
        IpAddr::V4(ip) if ip.is_unspecified() => Ipv4Addr::LOCALHOST.to_string(),
        IpAddr::V6(ip) if ip.is_unspecified() => Ipv4Addr::LOCALHOST.to_string(),
        // IPv6 字面量在 URL 中必须放进方括号
        IpAddr::V6(ip) => format!("[{ip}]"),
        IpAddr::V4(ip) => ip.to_string(),
    };
    format!("http://{host}:{port}")
}

/// 判断绑定地址是否为通配地址。
///
/// 通配地址接受所有网卡的连接，启动提示需要据此说明可用的访问方式。
///
/// 参数:
/// - `address`: 实际绑定的套接字地址
///
/// 返回:
/// - 地址为 0.0.0.0 或 :: 时返回 true
pub(super) fn is_wildcard(address: &SocketAddr) -> bool {
    address.ip().is_unspecified()
}

/// 按指定主机名组装访问地址。
///
/// 通配监听下浏览器无法直接使用 0.0.0.0，用户需要换成本机在目标网络里的地址；
/// 这里据此为提示信息生成示例链接。
///
/// 参数:
/// - `host`: 主机名或 IP 字面量
/// - `port`: 监听端口
/// - `token`: 本次启动的访问令牌
///
/// 返回:
/// - 带令牌的访问地址
pub(super) fn url_for_host(host: &str, port: u16, token: &str) -> String {
    // IPv6 字面量在 URL 中必须放进方括号，主机名与 IPv4 原样使用
    let host = match host.parse::<IpAddr>() {
        Ok(IpAddr::V6(ip)) => format!("[{ip}]"),
        _ => host.to_string(),
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
        assert_eq!(
            resolve_bind_address("  LocalHost  ", 4096).unwrap(),
            expected
        );
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
        assert_eq!(
            browsable_url_without_token(&"127.0.0.1:4096".parse().unwrap()),
            "http://127.0.0.1:4096"
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

    #[test]
    fn detects_wildcard_addresses() {
        assert!(is_wildcard(&"0.0.0.0:4096".parse().unwrap()));
        assert!(is_wildcard(&"[::]:4096".parse().unwrap()));
        assert!(!is_wildcard(&"127.0.0.1:4096".parse().unwrap()));
        assert!(!is_wildcard(&"192.168.1.10:4096".parse().unwrap()));
    }

    #[test]
    fn builds_urls_for_a_given_host() {
        assert_eq!(
            url_for_host("100.70.178.16", 4096, "abc"),
            "http://100.70.178.16:4096/?token=abc"
        );
        // IPv6 字面量需要方括号，主机名保持原样
        assert_eq!(
            url_for_host("fd7a::1", 4096, "abc"),
            "http://[fd7a::1]:4096/?token=abc"
        );
        assert_eq!(
            url_for_host("my-host", 4096, "abc"),
            "http://my-host:4096/?token=abc"
        );
    }
}
