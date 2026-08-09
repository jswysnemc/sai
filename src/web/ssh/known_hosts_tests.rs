use super::known_hosts::{append_known_host, check_known_host, HostKey, KnownHostStatus};
use crate::config::DEFAULT_SSH_PORT;

/// 构造测试用主机密钥。
fn key(hostname: &str, port: u16, key_base64: &str) -> HostKey {
    HostKey {
        hostname: hostname.to_string(),
        port,
        algorithm: "ssh-ed25519".to_string(),
        key_base64: key_base64.to_string(),
        fingerprint: "SHA256:abc".to_string(),
    }
}

#[test]
fn host_field_omits_the_default_port() {
    assert_eq!(key("example.com", DEFAULT_SSH_PORT, "AAA").host_field(), "example.com");
}

#[test]
fn host_field_brackets_a_custom_port() {
    assert_eq!(key("example.com", 2222, "AAA").host_field(), "[example.com]:2222");
}

#[test]
fn matching_entry_is_known() {
    let text = "example.com ssh-ed25519 AAA\n";
    assert_eq!(
        check_known_host(text, &key("example.com", DEFAULT_SSH_PORT, "AAA")),
        KnownHostStatus::Known
    );
}

#[test]
fn absent_entry_is_unknown() {
    let text = "other.com ssh-ed25519 AAA\n";
    assert_eq!(
        check_known_host(text, &key("example.com", DEFAULT_SSH_PORT, "AAA")),
        KnownHostStatus::Unknown
    );
}

#[test]
fn different_key_for_the_same_host_and_algorithm_is_changed() {
    let text = "example.com ssh-ed25519 STORED\n";
    assert_eq!(
        check_known_host(text, &key("example.com", DEFAULT_SSH_PORT, "AAA")),
        KnownHostStatus::Changed {
            stored_fingerprint: "STORED".to_string()
        }
    );
}

#[test]
fn a_different_algorithm_does_not_count_as_changed() {
    // 同一主机常同时登记 rsa 与 ed25519，算法不同不具备可比性
    let text = "example.com ssh-rsa OTHER\n";
    assert_eq!(
        check_known_host(text, &key("example.com", DEFAULT_SSH_PORT, "AAA")),
        KnownHostStatus::Unknown
    );
}

#[test]
fn matches_a_host_listed_among_comma_separated_names() {
    let text = "alias.example.com,example.com,192.0.2.10 ssh-ed25519 AAA\n";
    assert_eq!(
        check_known_host(text, &key("example.com", DEFAULT_SSH_PORT, "AAA")),
        KnownHostStatus::Known
    );
}

#[test]
fn distinguishes_entries_by_port() {
    let text = "[example.com]:2222 ssh-ed25519 AAA\n";
    assert_eq!(
        check_known_host(text, &key("example.com", 2222, "AAA")),
        KnownHostStatus::Known
    );
    // 默认端口与自定义端口是不同的记录
    assert_eq!(
        check_known_host(text, &key("example.com", DEFAULT_SSH_PORT, "AAA")),
        KnownHostStatus::Unknown
    );
}

#[test]
fn skips_comments_markers_and_malformed_lines() {
    let text = "# comment\n@revoked other.com ssh-ed25519 XXX\nbroken-line\n\nexample.com ssh-ed25519 AAA\n";
    assert_eq!(
        check_known_host(text, &key("example.com", DEFAULT_SSH_PORT, "AAA")),
        KnownHostStatus::Known
    );
}

#[test]
fn a_later_matching_entry_still_wins_over_an_earlier_mismatch() {
    // 主机轮换密钥后旧记录可能仍在，命中任一相同密钥即视为已知
    let text = "example.com ssh-ed25519 OLD\nexample.com ssh-ed25519 AAA\n";
    assert_eq!(
        check_known_host(text, &key("example.com", DEFAULT_SSH_PORT, "AAA")),
        KnownHostStatus::Known
    );
}

#[test]
fn append_adds_a_trailing_newline_to_unterminated_text() {
    let appended = append_known_host("example.com ssh-rsa OLD", &key("new.com", DEFAULT_SSH_PORT, "AAA"));
    assert_eq!(appended, "example.com ssh-rsa OLD\nnew.com ssh-ed25519 AAA\n");
}

#[test]
fn append_writes_the_first_entry_into_empty_text() {
    let appended = append_known_host("", &key("example.com", 2222, "AAA"));
    assert_eq!(appended, "[example.com]:2222 ssh-ed25519 AAA\n");
}

#[test]
fn appended_entry_is_recognized_afterwards() {
    let host_key = key("example.com", DEFAULT_SSH_PORT, "AAA");
    let appended = append_known_host("", &host_key);
    assert_eq!(check_known_host(&appended, &host_key), KnownHostStatus::Known);
}
