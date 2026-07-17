async fn collect_linux_evidence(
    args: &CheckIssueArgs,
    config: &DiagnosticsPluginConfig,
    report: &mut EvidenceReport,
) {
    linux_system_facts(config, report).await;
    match args.area {
        Area::System => linux_basic_checks(config, report).await,
        Area::App => linux_app_evidence(args, config, report).await,
        Area::InputMethod => linux_input_method_evidence(args, config, report).await,
        Area::Display => linux_display_evidence(args, config, report).await,
        Area::Audio => linux_audio_evidence(args, config, report).await,
        Area::Package => linux_package_evidence(args, config, report).await,
        Area::Gpu => linux_gpu_evidence(config, report).await,
        Area::Network => linux_network_evidence(config, report).await,
        Area::Storage => linux_storage_evidence(config, report).await,
    }
}

async fn collect_macos_evidence(
    args: &CheckIssueArgs,
    config: &DiagnosticsPluginConfig,
    report: &mut EvidenceReport,
) {
    fact_env(report, "env.shell", "SHELL");
    fact_env(report, "env.term", "TERM");
    fact_env(report, "env.lang", "LANG");
    let sw_vers = run_command(config, "sw_vers", &[], 2).await;
    push_log_if_stdout(report, "sw_vers", &sw_vers);
    if matches!(args.area, Area::App | Area::InputMethod) {
        if let Some(target) = args.target.as_deref() {
            command_exists_check(config, report, target).await;
            process_check(config, report, target).await;
        } else {
            report
                .missing_evidence
                .push("target app was not provided".to_string());
        }
    }
}

async fn linux_system_facts(config: &DiagnosticsPluginConfig, report: &mut EvidenceReport) {
    for key in [
        "SHELL",
        "TERM",
        "LANG",
        "XDG_SESSION_TYPE",
        "XDG_CURRENT_DESKTOP",
        "DESKTOP_SESSION",
        "WAYLAND_DISPLAY",
        "DISPLAY",
        "GTK_IM_MODULE",
        "QT_IM_MODULE",
        "QT_IM_MODULES",
        "XMODIFIERS",
        "SDL_IM_MODULE",
    ] {
        fact_env(report, &format!("env.{key}"), key);
    }
    if let Ok(text) = std::fs::read_to_string("/etc/os-release") {
        if let Some(name) = os_release_value(&text, "PRETTY_NAME") {
            report
                .facts
                .insert("os.pretty_name".to_string(), json!(name));
        }
    }
    let uname = run_command(config, "uname", &["-a"], 2).await;
    if !uname.stdout.trim().is_empty() {
        report
            .facts
            .insert("kernel.uname".to_string(), json!(uname.stdout.trim()));
    }
}

async fn linux_basic_checks(config: &DiagnosticsPluginConfig, report: &mut EvidenceReport) {
    for command in ["systemctl", "journalctl", "loginctl", "ip", "df"] {
        command_exists_check(config, report, command).await;
    }
}

async fn linux_app_evidence(
    args: &CheckIssueArgs,
    config: &DiagnosticsPluginConfig,
    report: &mut EvidenceReport,
) {
    let Some(target) = args.target.as_deref() else {
        report
            .missing_evidence
            .push("target app was not provided".to_string());
        return;
    };
    command_exists_check(config, report, target).await;
    process_check(config, report, target).await;
    if let Some(path) = command_path(config, target).await {
        report
            .facts
            .insert("app.command_path".to_string(), json!(path.clone()));
        package_owner(config, report, &path).await;
        app_probe_version(config, report, target).await;
    }
    recent_logs(args, config, report, &[target, "error", "failed"]).await;
}

async fn linux_input_method_evidence(
    args: &CheckIssueArgs,
    config: &DiagnosticsPluginConfig,
    report: &mut EvidenceReport,
) {
    for name in ["fcitx5", "ibus-daemon"] {
        process_check(config, report, name).await;
    }
    command_exists_check(config, report, "fcitx5-remote").await;
    if command_path(config, "fcitx5-remote").await.is_some() {
        let output = run_command(config, "fcitx5-remote", &[], 2).await;
        report.checks.push(Check {
            id: "input_method.fcitx5_remote".to_string(),
            status: if output.status == Some(0) {
                CheckStatus::Ok
            } else {
                CheckStatus::Warn
            },
            detail: "fcitx5-remote status probe".to_string(),
            evidence: compact_evidence(&output),
        });
    }

    let wayland_protocol = probe_wayland_protocol(config, report).await;

    let available_modules = scan_available_input_modules();
    report.facts.insert(
        "input_method.available_modules".to_string(),
        json!(available_modules.clone()),
    );

    let immodule_cache = read_gtk_immodule_cache();
    report.facts.insert(
        "input_method.immodule_cache".to_string(),
        json!(immodule_cache.clone()),
    );

    let Some(target) = args.target.as_deref() else {
        report.missing_evidence.push("target app was not provided; cannot check app toolkit, target environment, loaded .so modules, or path status".to_string());
        return;
    };
    let mut pids = process_check(config, report, target).await;
    if pids.is_empty() && args.allow_launch_probe {
        pids = launch_probe_target(args, config, report, target).await;
    }
    if pids.is_empty() {
        report.missing_evidence.push(format!(
            "target app {target} is not running; runtime environment and loaded .so modules are unavailable"
        ));
        report.recommended_next_probes.push(format!(
            "start {target}, then rerun check_issue with area=input_method and target={target}"
        ));
    }
    let target_env = pids.first().and_then(|pid| read_process_input_env(*pid));
    let loaded_modules = read_loaded_input_modules(&pids);

    let locale_info = probe_locale_info(config, report, &target_env).await;

    let socket_display_mode = probe_display_mode_via_sockets(config, report, &pids).await;

    let profile = build_input_method_profile(
        config,
        report,
        target,
        &pids,
        target_env,
        loaded_modules,
        available_modules,
        immodule_cache,
        wayland_protocol,
        locale_info,
        socket_display_mode,
    )
    .await;
    report
        .facts
        .insert("input_method.profile".to_string(), json!(profile));
    recent_logs(
        args,
        config,
        report,
        &[target, "fcitx", "ibus", "qt", "gtk", "xwayland"],
    )
    .await;
}

async fn probe_wayland_protocol(
    config: &DiagnosticsPluginConfig,
    report: &mut EvidenceReport,
) -> WaylandProtocolInfo {
    let wayland_info_output = run_command(config, "wayland-info", &[], 3).await;
    let wayland_info_available = wayland_info_output.status.is_some();
    let compositor_supports_text_input_v3 = wayland_info_output
        .stdout
        .contains("zwp_text_input_manager_v3");
    report.checks.push(Check {
        id: "input_method.wayland_text_input_v3".to_string(),
        status: if compositor_supports_text_input_v3 {
            CheckStatus::Ok
        } else {
            CheckStatus::Unknown
        },
        detail: "wayland-info: compositor text-input-v3 protocol support".to_string(),
        evidence: compact_evidence(&wayland_info_output),
    });

    let fcitx5_pids = process_ids(config, "fcitx5").await;
    let fcitx5_maps = fcitx5_pids
        .first()
        .and_then(|pid| std::fs::read_to_string(format!("/proc/{pid}/maps")).ok())
        .unwrap_or_default();
    let fcitx5_wayland_frontend_loaded = fcitx5_maps
        .lines()
        .any(|line| line.contains("libwaylandim.so") || line.contains("libwayland.so"));

    WaylandProtocolInfo {
        compositor_supports_text_input_v3,
        fcitx5_wayland_frontend_loaded,
        wayland_info_available,
    }
}

async fn probe_locale_info(
    config: &DiagnosticsPluginConfig,
    report: &mut EvidenceReport,
    target_env: &Option<BTreeMap<String, String>>,
) -> LocaleInfo {
    let target_lang = target_env.as_ref().and_then(|env| env.get("LANG").cloned());
    let target_lc_ctype = target_env
        .as_ref()
        .and_then(|env| env.get("LC_CTYPE").or_else(|| env.get("LC_ALL")).cloned());

    let locale_a_output = run_command(config, "locale", &["-a"], 2).await;
    let available_locales: Vec<String> = locale_a_output
        .stdout
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect();
    report.facts.insert(
        "input_method.available_locales".to_string(),
        json!(available_locales.clone()),
    );

    let check_locale = target_lc_ctype
        .as_deref()
        .or(target_lang.as_deref())
        .unwrap_or("C");
    let locale_valid = check_locale != "C"
        && check_locale != "POSIX"
        && available_locales.iter().any(|loc| {
            loc == check_locale
                || loc.eq_ignore_ascii_case(check_locale)
                || check_locale
                    .split('.')
                    .next()
                    .is_some_and(|prefix| loc.split('.').next() == Some(prefix))
        });

    LocaleInfo {
        target_lang,
        target_lc_ctype,
        available_locales,
        locale_valid,
    }
}

async fn probe_display_mode_via_sockets(
    config: &DiagnosticsPluginConfig,
    report: &mut EvidenceReport,
    pids: &[u32],
) -> DisplayMode {
    if pids.is_empty() {
        return DisplayMode::Unknown;
    }
    let ss_output = run_command(config, "ss", &["-xp"], 3).await;
    let unix_text = std::fs::read_to_string("/proc/net/unix").unwrap_or_default();

    let x11_inodes: BTreeSet<String> = unix_text
        .lines()
        .filter(|line| line.contains("X11-unix"))
        .filter_map(|line| line.split_whitespace().nth(7).map(|s| s.to_string()))
        .collect();
    let wayland_inodes: BTreeSet<String> = unix_text
        .lines()
        .filter(|line| line.contains("wayland"))
        .filter_map(|line| line.split_whitespace().nth(7).map(|s| s.to_string()))
        .collect();

    let mut has_x11 = false;
    let mut has_wayland = false;
    for pid in pids.iter().take(8) {
        let pid_str = format!("pid={pid}");
        for line in ss_output.stdout.lines() {
            if !line.contains(&pid_str) {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            for part in &parts {
                if part.chars().all(|c| c.is_ascii_digit()) && !part.is_empty() {
                    if x11_inodes.contains(*part) {
                        has_x11 = true;
                    }
                    if wayland_inodes.contains(*part) {
                        has_wayland = true;
                    }
                }
            }
        }
    }

    report.facts.insert(
        "input_method.socket_display_mode".to_string(),
        json!({
            "has_x11_socket": has_x11,
            "has_wayland_socket": has_wayland,
        }),
    );

    match (has_x11, has_wayland) {
        (true, _) => DisplayMode::XWayland,
        (false, true) => DisplayMode::WaylandNative,
        (false, false) => DisplayMode::Unknown,
    }
}

