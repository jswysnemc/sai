async fn linux_display_evidence(
    args: &CheckIssueArgs,
    config: &DiagnosticsPluginConfig,
    report: &mut EvidenceReport,
) {
    for service in [
        "xdg-desktop-portal.service",
        "pipewire.service",
        "wireplumber.service",
    ] {
        systemd_user_active_check(config, report, service).await;
    }
    process_check(config, report, "Xwayland").await;
    linux_gpu_evidence(config, report).await;
    recent_logs(
        args,
        config,
        report,
        &["portal", "pipewire", "wireplumber", "wayland", "xwayland"],
    )
    .await;
}

async fn linux_audio_evidence(
    args: &CheckIssueArgs,
    config: &DiagnosticsPluginConfig,
    report: &mut EvidenceReport,
) {
    for service in [
        "pipewire.service",
        "wireplumber.service",
        "pipewire-pulse.service",
    ] {
        systemd_user_active_check(config, report, service).await;
    }
    command_exists_check(config, report, "wpctl").await;
    if command_path(config, "wpctl").await.is_some() {
        let output = run_command(config, "wpctl", &["status"], 3).await;
        push_log_if_stdout(report, "wpctl status", &output);
    }
    recent_logs(
        args,
        config,
        report,
        &["pipewire", "wireplumber", "pulse", "audio"],
    )
    .await;
}

async fn linux_package_evidence(
    args: &CheckIssueArgs,
    config: &DiagnosticsPluginConfig,
    report: &mut EvidenceReport,
) {
    for command in ["pacman", "yay", "paru"] {
        command_exists_check(config, report, command).await;
    }
    report.facts.insert(
        "package.pacman_db_lock_exists".to_string(),
        json!(Path::new("/var/lib/pacman/db.lck").exists()),
    );
    recent_logs(
        args,
        config,
        report,
        &["pacman", "error", "failed", "warning"],
    )
    .await;
}

async fn linux_gpu_evidence(config: &DiagnosticsPluginConfig, report: &mut EvidenceReport) {
    command_exists_check(config, report, "lspci").await;
    if command_path(config, "lspci").await.is_some() {
        let output = run_command(config, "lspci", &["-nnk"], 4).await;
        let gpu = extract_lspci_gpu_blocks(&output.stdout);
        if !gpu.is_empty() {
            report.facts.insert("gpu.lspci".to_string(), json!(gpu));
        }
    }
    command_exists_check(config, report, "nvidia-smi").await;
}

async fn linux_network_evidence(config: &DiagnosticsPluginConfig, report: &mut EvidenceReport) {
    for command in ["ip", "resolvectl", "ping"] {
        command_exists_check(config, report, command).await;
    }
    if command_path(config, "ip").await.is_some() {
        let output = run_command(config, "ip", &["-brief", "addr"], 3).await;
        push_log(
            report,
            "ip -brief addr",
            &mask_network_addresses(&output.stdout),
        );
    }
    if command_path(config, "resolvectl").await.is_some() {
        let output = run_command(config, "resolvectl", &["status"], 3).await;
        push_log_if_stdout(report, "resolvectl status", &output);
    }
}

async fn linux_storage_evidence(config: &DiagnosticsPluginConfig, report: &mut EvidenceReport) {
    command_exists_check(config, report, "df").await;
    if command_path(config, "df").await.is_some() {
        let output = run_command(config, "df", &["-hT"], 3).await;
        push_log_if_stdout(report, "df -hT", &output);
    }
    command_exists_check(config, report, "btrfs").await;
}

async fn command_exists_check(
    config: &DiagnosticsPluginConfig,
    report: &mut EvidenceReport,
    name: &str,
) {
    let path = command_path(config, name).await;
    report.checks.push(Check {
        id: format!("command.{name}.exists"),
        status: if path.is_some() {
            CheckStatus::Ok
        } else {
            CheckStatus::Unknown
        },
        detail: if path.is_some() {
            format!("{name} is available")
        } else {
            format!("{name} is not available")
        },
        evidence: path.into_iter().collect(),
    });
}

async fn process_check(
    config: &DiagnosticsPluginConfig,
    report: &mut EvidenceReport,
    name: &str,
) -> Vec<u32> {
    let output = run_command(config, "pgrep", &["-af", name], 2).await;
    let matches = filtered_process_matches(&output.stdout, name);
    report.checks.push(Check {
        id: format!("process.{name}.running"),
        status: if matches.is_empty() {
            CheckStatus::Unknown
        } else {
            CheckStatus::Ok
        },
        detail: if matches.is_empty() {
            format!("no process matching {name} was found")
        } else {
            format!("process matching {name} is running")
        },
        evidence: if matches.is_empty() {
            Vec::new()
        } else {
            vec![clip(&matches.join("\n"), 1_000)]
        },
    });
    matches
        .iter()
        .filter_map(|line| line.split_whitespace().next()?.parse::<u32>().ok())
        .collect()
}

async fn launch_probe_target(
    args: &CheckIssueArgs,
    config: &DiagnosticsPluginConfig,
    report: &mut EvidenceReport,
    target: &str,
) -> Vec<u32> {
    if !safe_command_name(target) {
        report
            .missing_evidence
            .push("launch probe skipped because target command name is not safe".to_string());
        return Vec::new();
    }
    let before = process_ids(config, target).await;
    let spawn = Command::new(target)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    let Ok(child) = spawn else {
        report
            .missing_evidence
            .push(format!("failed to launch {target} for runtime sampling"));
        return Vec::new();
    };
    tokio::time::sleep(Duration::from_secs(args.launch_timeout_seconds)).await;
    let after = process_ids(config, target).await;
    let new_pids = after
        .iter()
        .copied()
        .filter(|pid| !before.contains(pid))
        .collect::<Vec<_>>();
    report.facts.insert(
        "launch_probe".to_string(),
        json!({"target": target, "launched_pid": child.id(), "pids_before": before, "pids_after": after, "new_pids": new_pids}),
    );
    if new_pids.is_empty() {
        after
    } else {
        new_pids
    }
}

async fn process_ids(config: &DiagnosticsPluginConfig, name: &str) -> Vec<u32> {
    let output = run_command(config, "pgrep", &["-af", name], 2).await;
    filtered_process_matches(&output.stdout, name)
        .iter()
        .filter_map(|line| line.split_whitespace().next()?.parse::<u32>().ok())
        .collect()
}

fn filtered_process_matches(output: &str, name: &str) -> Vec<String> {
    let name_lower = name.to_ascii_lowercase();
    let mut matches = output
        .lines()
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            lower.contains(&name_lower)
                && !lower.contains("pgrep -af")
                && !lower.contains("/usr/bin/bash -c")
                && !lower.contains("/bin/sh -c")
                && !line_starts_with_pid(line, std::process::id())
        })
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    matches.sort();
    matches
}

fn line_starts_with_pid(line: &str, pid: u32) -> bool {
    line.split_whitespace()
        .next()
        .and_then(|value| value.parse::<u32>().ok())
        == Some(pid)
}

fn read_process_input_env(pid: u32) -> Option<BTreeMap<String, String>> {
    let raw = std::fs::read(format!("/proc/{pid}/environ")).ok()?;
    let mut picked = BTreeMap::new();
    for item in raw.split(|byte| *byte == 0) {
        let entry = String::from_utf8_lossy(item);
        let Some((key, value)) = entry.split_once('=') else {
            continue;
        };
        if matches!(
            key,
            "GTK_IM_MODULE"
                | "QT_IM_MODULE"
                | "QT_IM_MODULES"
                | "XMODIFIERS"
                | "SDL_IM_MODULE"
                | "GLFW_IM_MODULE"
                | "XDG_SESSION_TYPE"
                | "WAYLAND_DISPLAY"
                | "DISPLAY"
                | "LANG"
                | "LC_ALL"
                | "LC_CTYPE"
        ) {
            picked.insert(key.to_string(), redact(value));
        }
    }
    Some(picked)
}

fn read_proc_cmdline(pid: u32) -> Option<String> {
    let raw = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    let parts = raw
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(|part| redact(String::from_utf8_lossy(part)))
        .collect::<Vec<_>>();
    (!parts.is_empty()).then(|| parts.join(" "))
}

fn read_loaded_input_modules(pids: &[u32]) -> Vec<String> {
    let mut modules = BTreeSet::new();
    for pid in pids.iter().take(8) {
        let Ok(text) = std::fs::read_to_string(format!("/proc/{pid}/maps")) else {
            continue;
        };
        for line in text.lines() {
            if let Some(path) = input_module_path_from_maps_line(line) {
                modules.insert(format!("pid {pid}: {path}"));
            }
        }
    }
    modules.into_iter().take(80).collect()
}

fn input_module_path_from_maps_line(line: &str) -> Option<String> {
    let path = line.split_whitespace().last()?;
    let lower = path.to_ascii_lowercase();
    let is_input_module = lower.contains("/immodules/")
        || lower.contains("im-fcitx")
        || lower.contains("im-xim")
        || lower.contains("im-ibus")
        || lower.contains("im-wayland")
        || lower.contains("platforminputcontext")
        || lower.contains("libibus")
        || lower.contains("libfcitx");
    (is_input_module && (lower.ends_with(".so") || lower.contains(".so."))).then(|| redact(path))
}

fn scan_available_input_modules() -> Vec<String> {
    let mut modules = BTreeSet::new();
    for root in ["/usr/lib", "/usr/lib64", "/app/lib"] {
        scan_available_input_modules_under(Path::new(root), 0, &mut modules);
    }
    modules.into_iter().take(120).collect()
}

fn scan_available_input_modules_under(dir: &Path, depth: usize, modules: &mut BTreeSet<String>) {
    if depth > 5 || modules.len() >= 120 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten().take(300) {
        let path = entry.path();
        let text = path.display().to_string();
        let lower = text.to_ascii_lowercase();
        if path.is_dir() {
            if lower.contains("gtk")
                || lower.contains("immodules")
                || lower.contains("qt")
                || lower.contains("fcitx")
                || lower.contains("ibus")
            {
                scan_available_input_modules_under(&path, depth + 1, modules);
            }
        } else if input_module_file_name(&lower) {
            modules.insert(redact(&text));
        }
    }
}

fn input_module_file_name(lower_path: &str) -> bool {
    (lower_path.contains("/immodules/")
        || lower_path.contains("im-fcitx")
        || lower_path.contains("im-xim")
        || lower_path.contains("im-ibus")
        || lower_path.contains("im-wayland")
        || lower_path.contains("platforminputcontext"))
        && (lower_path.ends_with(".so") || lower_path.contains(".so."))
}

fn linux_desktop_exec_for_target(target: &str) -> Option<String> {
    let mut dirs = Vec::new();
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(PathBuf::from(home).join(".local/share/applications"));
    }
    dirs.push(PathBuf::from("/usr/share/applications"));
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("desktop") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let exec = text.lines().find_map(|line| line.strip_prefix("Exec="));
            if path
                .file_stem()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case(target))
                || exec.is_some_and(|line| command_mentions_target(line, target))
            {
                return exec.map(redact);
            }
        }
    }
    None
}

fn command_mentions_target(line: &str, target: &str) -> bool {
    line.split(|ch: char| ch.is_whitespace() || ch == '/' || ch == '=')
        .any(|part| part == target)
}

fn infer_input_toolkit(text: &str) -> InputToolkit {
    let lower = text.to_ascii_lowercase();
    if lower.contains("qt_im_module")
        || lower.contains("platforminputcontext")
        || lower.contains("libqt")
    {
        InputToolkit::Qt
    } else if lower.contains("electron")
        || lower.contains("chromium")
        || lower.contains("chrome-sandbox")
        || lower.contains("steamwebhelper")
        || lower.contains("--ozone-platform")
        || lower.contains("linuxqq")
    {
        InputToolkit::ElectronChromium
    } else if lower.contains("gtk") || lower.contains("gdk") || lower.contains("immodules") {
        InputToolkit::Gtk
    } else if lower.contains("sdl") {
        InputToolkit::Sdl
    } else if lower.contains("java") {
        InputToolkit::Java
    } else if lower.contains("x11") || lower.contains("xlib") {
        InputToolkit::X11Legacy
    } else {
        InputToolkit::Unknown
    }
}

fn infer_display_mode(
    text: &str,
    env: Option<&BTreeMap<String, String>>,
    socket_mode: DisplayMode,
    loaded_modules: &[String],
) -> DisplayMode {
    let lower = text.to_ascii_lowercase();
    let has_ozone_wayland = lower.contains("--ozone-platform=wayland");

    if has_ozone_wayland {
        return DisplayMode::WaylandNative;
    }

    if socket_mode == DisplayMode::XWayland || socket_mode == DisplayMode::X11 {
        return DisplayMode::XWayland;
    }
    if socket_mode == DisplayMode::WaylandNative {
        return DisplayMode::WaylandNative;
    }

    let has_im_wayland = loaded_modules
        .iter()
        .any(|m| m.to_ascii_lowercase().contains("im-wayland"));
    if has_im_wayland {
        return DisplayMode::WaylandNative;
    }

    if let Some(env) = env {
        let has_wayland = env.get("WAYLAND_DISPLAY").is_some();
        let has_display = env.get("DISPLAY").is_some();
        return match (has_wayland, has_display) {
            (true, false) => DisplayMode::WaylandNative,
            (false, true) => DisplayMode::X11,
            (true, true) => DisplayMode::XWayland,
            _ => DisplayMode::Unknown,
        };
    }

    DisplayMode::Unknown
}

fn env_has(env: &BTreeMap<String, String>, key: &str, expected: &str) -> bool {
    env.get(key)
        .map(|value| value == expected || value.split(';').any(|item| item.trim() == expected))
        .unwrap_or(false)
}

fn loaded_module_evidence(loaded_modules: &[String], needles: &[&str]) -> Option<String> {
    loaded_modules.iter().find_map(|module| {
        let lower = module.to_ascii_lowercase();
        needles
            .iter()
            .any(|needle| lower.contains(&needle.to_ascii_lowercase()))
            .then(|| format!("runtime_loaded_module={module}"))
    })
}

fn available_module_evidence(available_modules: &[String], needles: &[&str]) -> Option<String> {
    available_modules.iter().find_map(|module| {
        let lower = module.to_ascii_lowercase();
        needles
            .iter()
            .any(|needle| lower.contains(&needle.to_ascii_lowercase()))
            .then(|| module.to_string())
    })
}

