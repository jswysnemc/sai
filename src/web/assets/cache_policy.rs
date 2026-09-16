use axum::http::{header, HeaderMap};

/// 【静态资源】【缓存策略】区分带哈希的构建资源和可更新入口。
/// 参数：`path` 为实际返回的资源路径；返回对应的 Cache-Control 值。
pub(super) fn cache_control(path: &str) -> &'static str {
    let hashed = path.starts_with("assets/")
        && path
            .rsplit_once('.')
            .and_then(|(stem, _)| stem.get(stem.len().saturating_sub(9)..))
            .is_some_and(|suffix| {
                suffix.len() == 9
                    && suffix.starts_with('-')
                    && suffix[1..]
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
            });
    if hashed {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    }
}

/// 【静态资源】【条件请求】使用弱比较校验实体标签，支持列表和通配符。
/// 参数：`headers` 为请求头，`etag` 为当前标签；返回是否可以使用客户端副本。
pub(super) fn is_not_modified(headers: &HeaderMap, etag: &str) -> bool {
    let expected = etag.strip_prefix("W/").unwrap_or(etag);
    headers
        .get_all(header::IF_NONE_MATCH)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .any(|candidate| {
            candidate == "*" || candidate.strip_prefix("W/").unwrap_or(candidate) == expected
        })
}

/// 【静态资源】【压缩协商】检查 gzip 是否可用，显式质量值优先于通配符。
/// 参数：`headers` 为请求头；返回客户端是否接受 gzip。
pub(super) fn accepts_gzip(headers: &HeaderMap) -> bool {
    let mut gzip = None;
    let mut wildcard = None;
    for value in headers.get_all(header::ACCEPT_ENCODING) {
        let Ok(value) = value.to_str() else { continue };
        for encoding in value.split(',') {
            let mut parts = encoding.split(';');
            let name = parts.next().unwrap_or_default().trim();
            let quality = parts
                .filter_map(|part| part.trim().split_once('='))
                .find(|(name, _)| name.trim().eq_ignore_ascii_case("q"))
                .map(|(_, value)| value.trim().parse::<f32>().unwrap_or(0.0))
                .unwrap_or(1.0);
            let accepted = quality > 0.0 && quality <= 1.0;
            if name.eq_ignore_ascii_case("gzip") {
                gzip = Some(accepted);
            } else if name == "*" {
                wildcard = Some(accepted);
            }
        }
    }
    gzip.or(wildcard).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【静态资源】【缓存测试】识别含连字符的哈希并保护入口；无参数，无返回值。
    #[test]
    fn cache_policy_only_marks_hashed_assets_immutable() {
        assert!(cache_control("assets/index-aB_12-34.js").contains("immutable"));
        assert_eq!(cache_control("index.html"), "no-cache");
        assert_eq!(cache_control("assets/settings.js"), "no-cache");
        assert_eq!(cache_control("material-icons/file.svg"), "no-cache");
    }

    /// 【静态资源】【协商测试】覆盖质量值、大小写和显式拒绝；无参数，无返回值。
    #[test]
    fn gzip_negotiation_respects_explicit_quality() {
        for (value, accepted) in [
            ("br, GZIP;q=0.5", true),
            ("*;q=1", true),
            ("gzip;q=0, *;q=1", false),
            ("gzip;q=invalid", false),
            ("br", false),
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(header::ACCEPT_ENCODING, value.parse().unwrap());
            assert_eq!(accepts_gzip(&headers), accepted, "{value}");
        }
    }

    /// 【静态资源】【标签测试】条件请求支持强弱标签混用；无参数，无返回值。
    #[test]
    fn entity_tags_use_weak_comparison() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::IF_NONE_MATCH,
            "\"old\", \"current\"".parse().unwrap(),
        );
        assert!(is_not_modified(&headers, "W/\"current\""));
        assert!(!is_not_modified(&headers, "W/\"changed\""));
        headers.insert(header::IF_NONE_MATCH, "*".parse().unwrap());
        assert!(is_not_modified(&headers, "W/\"changed\""));
    }
}
