use super::ssh_config_parser::parse_ssh_config;
use crate::config::DEFAULT_SSH_PORT;

#[test]
fn parses_a_basic_host_block() {
    let parsed = parse_ssh_config(
        "Host build\n  HostName build.example.com\n  User deploy\n  Port 2222\n  IdentityFile ~/.ssh/id_ed25519\n",
    );

    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].alias, "build");
    assert_eq!(parsed[0].hostname, "build.example.com");
    assert_eq!(parsed[0].username, "deploy");
    assert_eq!(parsed[0].port, 2222);
    assert_eq!(parsed[0].identity_file, "~/.ssh/id_ed25519");
}

#[test]
fn accepts_the_equals_form_and_mixed_case_keywords() {
    let parsed = parse_ssh_config("HOST=web\n  hostname=web.example.com\n  USER=root\n");

    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].hostname, "web.example.com");
    assert_eq!(parsed[0].username, "root");
}

#[test]
fn falls_back_to_the_alias_when_hostname_is_absent() {
    let parsed = parse_ssh_config("Host example.com\n  User deploy\n");

    assert_eq!(parsed[0].hostname, "example.com");
}

#[test]
fn defaults_the_port_when_missing_or_invalid() {
    let parsed = parse_ssh_config("Host a\n  Port not-a-number\nHost b\n  Port 0\n");

    assert_eq!(parsed[0].port, DEFAULT_SSH_PORT);
    assert_eq!(parsed[1].port, DEFAULT_SSH_PORT);
}

#[test]
fn skips_wildcard_and_negated_host_patterns() {
    let parsed = parse_ssh_config("Host *\n  User global\n\nHost !denied\n  User no\n\nHost real\n  HostName real.example.com\n");

    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].alias, "real");
}

#[test]
fn takes_the_first_concrete_alias_from_a_multi_alias_host_line() {
    let parsed = parse_ssh_config("Host prod * backup\n  HostName prod.example.com\n");

    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].alias, "prod");
}

#[test]
fn ignores_comments_and_blank_lines() {
    let parsed = parse_ssh_config("# leading comment\n\nHost box  # trailing\n  HostName box.example.com\n  # User commented\n");

    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].hostname, "box.example.com");
    assert_eq!(parsed[0].username, "");
}

#[test]
fn strips_quotes_around_the_identity_file() {
    let parsed = parse_ssh_config("Host box\n  IdentityFile \"~/.ssh/my key\"\n");

    assert_eq!(parsed[0].identity_file, "~/.ssh/my key");
}

#[test]
fn separates_consecutive_host_blocks() {
    let parsed = parse_ssh_config(
        "Host one\n  HostName one.example.com\n  User a\nHost two\n  HostName two.example.com\n  User b\n",
    );

    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].username, "a");
    assert_eq!(parsed[1].username, "b");
    // 第二段不应继承第一段的取值
    assert_eq!(parsed[1].hostname, "two.example.com");
}

#[test]
fn ignores_directives_before_any_host_block() {
    let parsed = parse_ssh_config(
        "ServerAliveInterval 60\nUser orphan\nHost box\n  HostName box.example.com\n",
    );

    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].username, "");
}

#[test]
fn returns_nothing_for_empty_input() {
    assert!(parse_ssh_config("").is_empty());
    assert!(parse_ssh_config("# only comments\n\n").is_empty());
}
