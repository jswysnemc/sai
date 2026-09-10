use anyhow::{bail, Context, Result};
use reqwest::{ClientBuilder, Url};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

/// 【插件下载】【地址校验】仅接受没有内嵌凭据的 HTTP(S) URL，错误不包含原始地址。
/// @param value 原始下载地址
/// @returns 规范 URL
pub(super) fn parse(value: &str) -> Result<Url> {
    if value.len() > 8192 {
        bail!("plugin download URL exceeds size limit");
    }
    let url = Url::parse(value).context("invalid plugin download URL")?;
    if url.as_str().len() > 8192
        || !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        bail!("plugin download requires an HTTP(S) URL without credentials");
    }
    Ok(url)
}

/// 【插件下载】【解析固定】逐次校验全部 DNS 结果，再固定实际连接地址，禁用代理绕过校验。
/// @param builder 未构建客户端；url 为本次跳转目标
/// @returns 只能连接已检查公开地址的客户端构造器
pub(super) async fn pin(builder: ClientBuilder, url: &Url) -> Result<ClientBuilder> {
    let host = url.host_str().context("plugin download host is missing")?;
    let literal = host.trim_matches(['[', ']']).parse::<IpAddr>().ok();
    let port = url
        .port_or_known_default()
        .context("plugin download port is missing")?;
    let addresses: Vec<SocketAddr> = match literal {
        Some(ip) => vec![SocketAddr::new(ip, port)],
        None => tokio::net::lookup_host((host, port))
            .await
            .context("resolve plugin download host")?
            .take(65)
            .collect(),
    };
    validate(&addresses)?;
    let builder = builder.no_proxy();
    Ok(if literal.is_none() {
        builder.resolve_to_addrs(host, &addresses)
    } else {
        builder
    })
}

/// 【插件下载】【公开集合】混合公开与私有解析、空结果和异常地址数量都不能作为公开下载。
/// @param addresses 一次解析的完整有界结果
/// @returns 全部结果均为公开单播地址时成功
fn validate(addresses: &[SocketAddr]) -> Result<()> {
    if addresses.is_empty()
        || addresses.len() > 64
        || addresses.iter().any(|address| !is_public(address.ip()))
    {
        bail!("plugin public download resolved to a non-public or invalid address set");
    }
    Ok(())
}

/// 【插件下载】【公网分类】排除本地、私有、保留、文档、组播及地址转换范围。
/// @param address 待连接 IP
/// @returns 可以用于匿名公开下载时为 true
fn is_public(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(ip) => public_v4(ip),
        IpAddr::V6(ip) => {
            if let Some(ip) = ip.to_ipv4_mapped() {
                return public_v4(ip);
            }
            let parts = ip.segments();
            (parts[0] & 0xe000) == 0x2000
                && !(parts[0] == 0x2001 && (parts[1] < 0x0200 || parts[1] == 0x0db8))
                && parts[0] != 0x2002
                && !(parts[0] == 0x3fff && parts[1] < 0x1000)
        }
    }
}

/// 【插件下载】【IPv4 分类】拒绝 IANA 特殊用途地址以及所有非单播地址。
/// @param address IPv4 地址
/// @returns 不属于特殊用途前缀时为 true
fn public_v4(address: Ipv4Addr) -> bool {
    let [a, b, c, _] = address.octets();
    !(matches!(a, 0 | 10 | 127)
        || a >= 224
        || (a == 100 && (64..=127).contains(&b))
        || (a == 169 && b == 254)
        || (a == 172 && (16..=31).contains(&b))
        || (a == 192 && (b == 168 || (b == 0 && matches!(c, 0 | 2)) || (b == 88 && c == 99)))
        || (a == 198 && (matches!(b, 18 | 19) || (b == 51 && c == 100)))
        || (a == 203 && b == 0 && c == 113))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【下载测试】【地址范围】公开下载不能访问私有、保留或 IPv4 嵌入的本地地址。
    #[test]
    fn private_reserved_and_transition_addresses_are_rejected() {
        for ip in [
            "0.0.0.0",
            "10.1.2.3",
            "100.64.0.1",
            "127.0.0.1",
            "169.254.169.254",
            "172.16.0.1",
            "192.168.1.1",
            "192.0.2.1",
            "198.18.1.1",
            "198.51.100.2",
            "203.0.113.1",
            "224.0.0.1",
            "255.255.255.255",
            "::",
            "::1",
            "fe80::1",
            "fc00::1",
            "ff02::1",
            "::ffff:127.0.0.1",
            "64:ff9b::7f00:1",
            "2001:db8::1",
            "2001::1",
            "2002:7f00:1::",
            "3fff::1",
        ] {
            assert!(!is_public(ip.parse().unwrap()), "{ip}");
        }
        for ip in [
            "1.1.1.1",
            "8.8.8.8",
            "104.16.1.1",
            "2606:4700:4700::1111",
            "2001:4860:4860::8888",
        ] {
            assert!(is_public(ip.parse().unwrap()), "{ip}");
        }
    }

    /// 【下载测试】【解析集合】同一域名存在私有备用地址时也不能通过公开网络授权。
    #[test]
    fn mixed_dns_results_are_rejected() {
        assert!(validate(&[
            "1.1.1.1:443".parse().unwrap(),
            "127.0.0.1:443".parse().unwrap()
        ])
        .is_err());
        assert!(validate(&[]).is_err());
    }
}
