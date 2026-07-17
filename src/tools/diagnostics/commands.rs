async fn systemd_user_active_check(
    config: &DiagnosticsPluginConfig,
    report: &mut EvidenceReport,
    service: &str,
) {
    let output = run_command(config, "systemctl", &["--user", "is-active", service], 2).await;
    report.checks.push(Check {
        id: format!("systemd_user.{service}.active"),
        status: if output.status == Some(0) {
            CheckStatus::Ok
        } else {
            CheckStatus::Warn
        },
        detail: format!("systemctl --user is-active {service}"),
        evidence: compact_evidence(&output),
    });
}

async fn app_probe_version(
    config: &DiagnosticsPluginConfig,
    report: &mut EvidenceReport,
    command: &str,
) {
    if !safe_command_name(command) {
        return;
    }
    let output = run_command(config, command, &["--version"], 2).await;
    push_log_if_stdout(report, &format!("{command} --version"), &output);
}

async fn package_owner(config: &DiagnosticsPluginConfig, report: &mut EvidenceReport, path: &str) {
    if command_path(config, "pacman").await.is_none() {
        return;
    }
    let output = run_command(config, "pacman", &["-Qo", path], 3).await;
    push_log_if_stdout(report, "pacman -Qo", &output);
}

fn package_probe_for_command(
    config: &DiagnosticsPluginConfig,
    command_path: &str,
    target: &str,
) -> Option<String> {
    let owner = std::process::Command::new("pacman")
        .args(["-Qo", command_path])
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    let owner_text = String::from_utf8_lossy(&owner.stdout);
    let package = package_name_from_pacman_owner(&owner_text)?;
    if !safe_command_name(&package) {
        return None;
    }
    let output = std::process::Command::new("pacman")
        .args(["-Ql", &package])
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    let mut lines = vec![format!("package={package}"), format!("target={target}")];
    lines.extend(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| package_probe_line(line))
            .take(80)
            .map(ToString::to_string),
    );
    Some(redact(&clip(
        &lines.join("\n"),
        config.max_stdout_chars.min(4_000),
    )))
}

fn package_name_from_pacman_owner(text: &str) -> Option<String> {
    let parts = text.split_whitespace().collect::<Vec<_>>();
    if let Some(index) = parts.iter().position(|part| *part == "by" || *part == "由") {
        return parts.get(index + 1).map(|value| value.to_string());
    }
    None
}

fn package_probe_line(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains("libgtk")
        || lower.contains("libgdk")
        || lower.contains("libqt")
        || lower.contains("platforminputcontext")
        || lower.contains("immodules")
        || lower.contains("electron")
        || lower.contains("chrome")
        || lower.ends_with(".desktop")
        || lower.contains("/bin/")
}

async fn recent_logs(
    args: &CheckIssueArgs,
    config: &DiagnosticsPluginConfig,
    report: &mut EvidenceReport,
    needles: &[&str],
) {
    if args.depth == Depth::Quick || command_path(config, "journalctl").await.is_none() {
        return;
    }
    let since = format!("-{}min", args.recent_minutes);
    let output = run_command(
        config,
        "journalctl",
        &["--user", "--since", &since, "--no-pager", "-n", "200"],
        5,
    )
    .await;
    let text = output
        .stdout
        .lines()
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            needles
                .iter()
                .any(|needle| lower.contains(&needle.to_ascii_lowercase()))
        })
        .take(80)
        .collect::<Vec<_>>()
        .join("\n");
    push_log(report, "journalctl --user recent filtered", &text);
}

async fn command_path(config: &DiagnosticsPluginConfig, command: &str) -> Option<String> {
    if !safe_command_name(command) {
        return None;
    }
    let output = run_command(config, "which", &[command], 2).await;
    (output.status == Some(0))
        .then(|| {
            output
                .stdout
                .lines()
                .next()
                .unwrap_or_default()
                .trim()
                .to_string()
        })
        .filter(|value| !value.is_empty())
}

async fn run_command(
    config: &DiagnosticsPluginConfig,
    command: &str,
    args: &[&str],
    timeout_seconds: u64,
) -> ProbeOutput {
    if !safe_command_name(command) {
        return ProbeOutput {
            status: None,
            stdout: String::new(),
            stderr: String::new(),
            timed_out: false,
        };
    }
    let result = timeout(
        Duration::from_secs(timeout_seconds.min(config.command_timeout_seconds).max(1)),
        Command::new(command)
            .args(args)
            .stdin(Stdio::null())
            .output(),
    )
    .await;
    match result {
        Ok(Ok(output)) => ProbeOutput {
            status: output.status.code(),
            stdout: clip(
                &String::from_utf8_lossy(&output.stdout),
                config.max_stdout_chars,
            ),
            stderr: clip(
                &String::from_utf8_lossy(&output.stderr),
                config.max_stderr_chars,
            ),
            timed_out: false,
        },
        Ok(Err(err)) => ProbeOutput {
            status: None,
            stdout: String::new(),
            stderr: err.to_string(),
            timed_out: false,
        },
        Err(_) => ProbeOutput {
            status: None,
            stdout: String::new(),
            stderr: String::new(),
            timed_out: true,
        },
    }
}

fn fact_env(report: &mut EvidenceReport, key: &str, env: &str) {
    if let Ok(value) = std::env::var(env) {
        if !value.trim().is_empty() {
            report.facts.insert(key.to_string(), json!(redact(&value)));
        }
    }
}

fn push_log_if_stdout(report: &mut EvidenceReport, source: &str, output: &ProbeOutput) {
    if !output.stdout.trim().is_empty() {
        push_log(report, source, &output.stdout);
    }
}

fn push_log(report: &mut EvidenceReport, source: &str, message: &str) {
    if !message.trim().is_empty() {
        report.logs.push(LogExcerpt {
            source: source.to_string(),
            message: clip(message, 2_000),
        });
    }
}

fn compact_evidence(output: &ProbeOutput) -> Vec<String> {
    let mut evidence = Vec::new();
    if let Some(status) = output.status {
        evidence.push(format!("exit={status}"));
    }
    if !output.stdout.trim().is_empty() {
        evidence.push(format!("stdout={}", clip(&output.stdout, 800)));
    }
    if !output.stderr.trim().is_empty() {
        evidence.push(format!("stderr={}", clip(&output.stderr, 800)));
    }
    if output.timed_out {
        evidence.push("timed_out=true".to_string());
    }
    evidence
}

fn extract_lspci_gpu_blocks(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            lower.contains("vga")
                || lower.contains("3d controller")
                || lower.contains("display controller")
                || lower.contains("kernel driver in use")
        })
        .take(80)
        .map(redact)
        .collect()
}

fn os_release_value(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let (name, value) = line.split_once('=')?;
        (name == key).then(|| value.trim_matches('"').to_string())
    })
}

fn mask_network_addresses(text: &str) -> String {
    text.split_whitespace()
        .map(|part| {
            if part.contains('.') && part.chars().any(|ch| ch.is_ascii_digit()) {
                "<ipv4>".to_string()
            } else if part.contains(':') && part.chars().any(|ch| ch.is_ascii_hexdigit()) {
                "<ipv6-or-mac>".to_string()
            } else {
                part.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn redact(value: impl AsRef<str>) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    if home.is_empty() {
        value.as_ref().to_string()
    } else {
        value.as_ref().replace(&home, "$HOME")
    }
}

fn clip(value: &str, max_chars: usize) -> String {
    let value = value.trim();
    if value.chars().count() <= max_chars {
        value.to_string()
    } else {
        format!(
            "{}...",
            value
                .chars()
                .take(max_chars.saturating_sub(3))
                .collect::<String>()
        )
    }
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn safe_command_name(value: &str) -> bool {
    !value.trim().is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '+'))
}

