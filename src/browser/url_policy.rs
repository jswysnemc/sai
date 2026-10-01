//! 地址策略：补全协议，只放行网页协议，拦截本地文件与浏览器内部页面。

use anyhow::{bail, Result};

/// 允许打开的协议前缀。
const ALLOWED_SCHEMES: &[&str] = &["http", "https"];
/// 无协议输入时视为本机地址、使用 http 的主机名。
const LOCAL_HOSTS: &[&str] = &["localhost", "127.0.0.1", "0.0.0.0", "[::1]"];

/// 【内置浏览器】【地址规范化】补全协议并校验地址是否允许打开。
///
/// 用户在地址栏输入不含空格也不像主机名的文本时，按搜索词交给搜索引擎。
///
/// @param input 为用户或模型给出的地址；allow_search 为是否把非地址文本转为搜索
/// @returns 可交给浏览器导航的完整地址
pub(crate) fn normalize_url(input: &str, allow_search: bool) -> Result<String> {
    let input = input.trim();
    if input.is_empty() {
        bail!("url is required");
    }
    if input == "about:blank" {
        return Ok(input.to_string());
    }
    // 1. 已带协议时只校验协议是否放行
    if let Some((scheme, _)) = input.split_once(':') {
        let scheme = scheme.to_ascii_lowercase();
        if input[scheme.len()..].starts_with("://") {
            if ALLOWED_SCHEMES.contains(&scheme.as_str()) {
                return Ok(input.to_string());
            }
            bail!("scheme {scheme}: is not allowed in the built-in browser; use http or https");
        }
        if is_blocked_scheme(&scheme) {
            bail!("scheme {scheme}: is not allowed in the built-in browser");
        }
    }
    // 2. 无协议时按主机名补全；本机地址默认 http，其余默认 https
    if looks_like_host(input) {
        let host = input
            .split(['/', '?', '#'])
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let bare_host = host.rsplit_once(':').map_or(host.as_str(), |(name, port)| {
            if port.chars().all(|c| c.is_ascii_digit()) {
                name
            } else {
                host.as_str()
            }
        });
        let scheme = if LOCAL_HOSTS.contains(&bare_host) || is_private_ipv4(bare_host) {
            "http"
        } else {
            "https"
        };
        return Ok(format!("{scheme}://{input}"));
    }
    // 3. 其余文本按搜索词处理
    if allow_search {
        return Ok(format!(
            "https://duckduckgo.com/?q={}",
            urlencoding::encode(input)
        ));
    }
    bail!("not a valid http(s) url: {input}")
}

/// 【内置浏览器】【协议拦截】判断无 `//` 形式的协议是否需要拦截。
/// @param scheme 为小写协议名
/// @returns 需要拦截时为 true
fn is_blocked_scheme(scheme: &str) -> bool {
    matches!(
        scheme,
        "javascript" | "file" | "chrome" | "devtools" | "view-source" | "data" | "blob"
    )
}

/// 【内置浏览器】【主机判断】判断无协议输入是否像主机名或主机加路径。
/// @param input 为去除首尾空白的输入
/// @returns 像地址时为 true
fn looks_like_host(input: &str) -> bool {
    if input.contains(char::is_whitespace) {
        return false;
    }
    let host = input.split(['/', '?', '#']).next().unwrap_or_default();
    let name = host.rsplit_once(':').map_or(host, |(name, _)| name);
    LOCAL_HOSTS.contains(&name.to_ascii_lowercase().as_str())
        || (name.contains('.')
            && !name.starts_with('.')
            && !name.ends_with('.')
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.'))
}

/// 【内置浏览器】【内网判断】判断主机是否为私有 IPv4 地址。
/// @param host 为主机名
/// @returns 私有地址时为 true
fn is_private_ipv4(host: &str) -> bool {
    host.parse::<std::net::Ipv4Addr>()
        .is_ok_and(|ip| ip.is_private() || ip.is_loopback())
}
