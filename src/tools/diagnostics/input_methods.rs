fn read_gtk_immodule_cache() -> Vec<ImmoduleCacheEntry> {
    let mut entries = Vec::new();
    for cache_path in [
        "/usr/lib/gtk-3.0/3.0.0/immodules.cache",
        "/usr/lib/gtk-4.0/4.0.0/immodules.cache",
    ] {
        let Ok(text) = std::fs::read_to_string(cache_path) else {
            continue;
        };
        for line in text.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 5 {
                continue;
            }
            let so_path = parts[0].trim_matches('"').to_string();
            let module_name = parts[1].trim_matches('"').to_string();
            let locales = parts
                .get(4)
                .map(|s| s.trim_matches('"').to_string())
                .unwrap_or_default();
            entries.push(ImmoduleCacheEntry {
                so_path,
                module_name,
                locales,
            });
        }
    }
    entries
}

async fn build_input_method_profile(
    config: &DiagnosticsPluginConfig,
    report: &mut EvidenceReport,
    target: &str,
    pids: &[u32],
    target_env: Option<BTreeMap<String, String>>,
    loaded_modules: Vec<String>,
    available_modules: Vec<String>,
    immodule_cache: Vec<ImmoduleCacheEntry>,
    wayland_protocol: WaylandProtocolInfo,
    locale_info: LocaleInfo,
    socket_display_mode: DisplayMode,
) -> InputMethodProfile {
    let command_line = pids.first().and_then(|pid| read_proc_cmdline(*pid));
    let desktop_exec = linux_desktop_exec_for_target(target);
    let command_path = command_path(config, target).await;
    let package_probe = command_path
        .as_deref()
        .and_then(|path| package_probe_for_command(config, path, target));
    if let Some(probe) = &package_probe {
        report
            .facts
            .insert("input_method.package_probe".to_string(), json!(probe));
    }
    let evidence_text = [
        target.to_string(),
        command_line.clone().unwrap_or_default(),
        desktop_exec.clone().unwrap_or_default(),
        command_path.unwrap_or_default(),
        package_probe.unwrap_or_default(),
    ]
    .join(" ");
    let raw_toolkit = infer_input_toolkit(&evidence_text);
    let display_mode = infer_display_mode(
        &evidence_text,
        target_env.as_ref(),
        socket_display_mode,
        &loaded_modules,
    );
    let toolkit = refine_electron_toolkit(raw_toolkit, display_mode);
    let path_status = input_method_path_status(
        toolkit,
        display_mode,
        !pids.is_empty() && command_line.is_some(),
        target_env.as_ref(),
        &loaded_modules,
        &available_modules,
        &immodule_cache,
        &wayland_protocol,
        &locale_info,
    );
    InputMethodProfile {
        toolkit,
        display_mode,
        runtime_observed: !pids.is_empty() && command_line.is_some(),
        command_line,
        desktop_exec,
        target_env,
        loaded_input_modules: loaded_modules,
        available_input_modules: available_modules,
        immodule_cache,
        wayland_protocol,
        locale_info,
        path_status,
    }
}

fn refine_electron_toolkit(toolkit: InputToolkit, display_mode: DisplayMode) -> InputToolkit {
    match toolkit {
        InputToolkit::ElectronChromium => match display_mode {
            DisplayMode::WaylandNative => InputToolkit::ElectronWayland,
            DisplayMode::X11 | DisplayMode::XWayland => InputToolkit::ElectronX11,
            DisplayMode::Unknown => InputToolkit::ElectronX11,
        },
        other => other,
    }
}

fn input_method_path_status(
    toolkit: InputToolkit,
    display_mode: DisplayMode,
    runtime_observed: bool,
    env: Option<&BTreeMap<String, String>>,
    loaded_modules: &[String],
    available_modules: &[String],
    immodule_cache: &[ImmoduleCacheEntry],
    wayland_protocol: &WaylandProtocolInfo,
    locale_info: &LocaleInfo,
) -> InputMethodPathStatus {
    let mut paths = Vec::new();
    let mut evidence = Vec::new();
    let mut missing = Vec::new();

    evidence.push(format!("toolkit={toolkit:?}"));
    evidence.push(format!("display_mode={display_mode:?}"));
    if !runtime_observed {
        missing.push("runtime process evidence".to_string());
    }
    if toolkit == InputToolkit::Unknown {
        missing.push("app toolkit/framework evidence".to_string());
    }
    paths.push(NamedPathCheck {
        name: "app_adapter".to_string(),
        status: if missing.is_empty() {
            "confirmed"
        } else {
            "unknown"
        }
        .to_string(),
        evidence,
        missing,
    });

    let relevant_paths = relevant_input_paths(toolkit);
    for path_name in &relevant_paths {
        let check = check_single_path(
            path_name,
            env,
            loaded_modules,
            available_modules,
            immodule_cache,
            wayland_protocol,
            locale_info,
        );
        paths.push(check);
    }

    let any_confirmed = paths.iter().skip(1).any(|p| p.status == "confirmed");
    let all_incomplete = paths
        .iter()
        .skip(1)
        .all(|p| p.status == "missing" || p.status == "unknown");
    let overall = if any_confirmed {
        "path_evidence_complete".to_string()
    } else if all_incomplete {
        "path_evidence_incomplete".to_string()
    } else {
        "path_evidence_partial".to_string()
    };

    InputMethodPathStatus { paths, overall }
}

fn relevant_input_paths(toolkit: InputToolkit) -> Vec<&'static str> {
    match toolkit {
        InputToolkit::Gtk | InputToolkit::Qt | InputToolkit::Sdl | InputToolkit::X11Legacy => {
            vec!["wayland_protocol", "toolkit_module", "xim"]
        }
        InputToolkit::ElectronX11 => vec!["gtk_module", "xim"],
        InputToolkit::ElectronWayland => vec!["wayland_protocol", "gtk_module"],
        InputToolkit::ElectronChromium => vec!["wayland_protocol", "gtk_module", "xim"],
        InputToolkit::Java => vec!["xim"],
        InputToolkit::Unknown => vec![
            "wayland_protocol",
            "gtk_module",
            "qt_module",
            "sdl_module",
            "xim",
        ],
    }
}

#[allow(clippy::too_many_arguments)]
fn check_single_path(
    path_name: &str,
    env: Option<&BTreeMap<String, String>>,
    loaded_modules: &[String],
    available_modules: &[String],
    immodule_cache: &[ImmoduleCacheEntry],
    wayland_protocol: &WaylandProtocolInfo,
    locale_info: &LocaleInfo,
) -> NamedPathCheck {
    let loaded = |needles: &[&str]| loaded_module_evidence(loaded_modules, needles);
    let available = |needles: &[&str]| available_module_evidence(available_modules, needles);

    match path_name {
        "wayland_protocol" => {
            let mut ev = Vec::new();
            let mut miss = Vec::new();
            if wayland_protocol.compositor_supports_text_input_v3 {
                ev.push("compositor supports zwp_text_input_manager_v3".to_string());
            } else {
                miss.push("compositor text-input-v3 protocol support".to_string());
            }
            if wayland_protocol.fcitx5_wayland_frontend_loaded {
                ev.push("fcitx5 loaded libwaylandim.so (wayland frontend)".to_string());
            } else {
                miss.push("fcitx5 wayland frontend (libwaylandim.so)".to_string());
            }
            let status = if wayland_protocol.compositor_supports_text_input_v3
                && wayland_protocol.fcitx5_wayland_frontend_loaded
            {
                "confirmed"
            } else if wayland_protocol.compositor_supports_text_input_v3
                || wayland_protocol.fcitx5_wayland_frontend_loaded
            {
                "configured"
            } else {
                "missing"
            };
            NamedPathCheck {
                name: "wayland_protocol".to_string(),
                status: status.to_string(),
                evidence: ev,
                missing: miss,
            }
        }
        "gtk_module" | "toolkit_module" => {
            let mut ev = Vec::new();
            let mut miss = Vec::new();

            if let Some(env) = env {
                if path_name == "gtk_module" {
                    if let Some(value) = env.get("GTK_IM_MODULE").filter(|v| !v.trim().is_empty()) {
                        ev.push(format!("GTK_IM_MODULE={value}"));
                    }
                }
            }

            if let Some(item) = loaded(&["im-fcitx", "im-wayland", "im-xim", "im-ibus"]) {
                ev.push(item);
                NamedPathCheck {
                    name: path_name.to_string(),
                    status: "confirmed".to_string(),
                    evidence: ev,
                    missing: miss,
                }
            } else if let Some(item) = available(&["im-fcitx", "im-wayland", "im-xim", "im-ibus"]) {
                ev.push(format!("available_on_disk={item}"));
                let locale_match =
                    check_immodule_locale(path_name, "fcitx", immodule_cache, locale_info);
                if !locale_match.is_empty() {
                    ev.push(locale_match);
                }
                NamedPathCheck {
                    name: path_name.to_string(),
                    status: "configured".to_string(),
                    evidence: ev,
                    missing: miss,
                }
            } else {
                miss.push("GTK input module .so (neither loaded nor on disk)".to_string());
                NamedPathCheck {
                    name: path_name.to_string(),
                    status: "missing".to_string(),
                    evidence: ev,
                    missing: miss,
                }
            }
        }
        "qt_module" => {
            let mut ev = Vec::new();
            let mut miss = Vec::new();
            if let Some(env) = env {
                if let Some(value) = env.get("QT_IM_MODULE").filter(|v| !v.trim().is_empty()) {
                    ev.push(format!("QT_IM_MODULE={value}"));
                }
                if let Some(value) = env.get("QT_IM_MODULES").filter(|v| !v.trim().is_empty()) {
                    ev.push(format!("QT_IM_MODULES={value}"));
                }
            }
            if let Some(item) = loaded(&["platforminputcontext", "libfcitx", "libibus"]) {
                ev.push(item);
                NamedPathCheck {
                    name: "qt_module".to_string(),
                    status: "confirmed".to_string(),
                    evidence: ev,
                    missing: miss,
                }
            } else if let Some(item) = available(&["platforminputcontext", "fcitx"]) {
                ev.push(format!("available_on_disk={item}"));
                NamedPathCheck {
                    name: "qt_module".to_string(),
                    status: "configured".to_string(),
                    evidence: ev,
                    missing: miss,
                }
            } else {
                miss.push("Qt platforminputcontext .so evidence".to_string());
                NamedPathCheck {
                    name: "qt_module".to_string(),
                    status: "missing".to_string(),
                    evidence: ev,
                    missing: miss,
                }
            }
        }
        "sdl_module" => {
            let mut ev = Vec::new();
            let mut miss = Vec::new();
            if let Some(env) = env {
                if let Some(value) = env.get("SDL_IM_MODULE").filter(|v| !v.trim().is_empty()) {
                    ev.push(format!("SDL_IM_MODULE={value}"));
                }
            }
            if let Some(item) = loaded(&["libfcitx", "libibus", "sdl"]) {
                ev.push(item);
                NamedPathCheck {
                    name: "sdl_module".to_string(),
                    status: "confirmed".to_string(),
                    evidence: ev,
                    missing: miss,
                }
            } else {
                miss.push("SDL input bridge .so evidence".to_string());
                NamedPathCheck {
                    name: "sdl_module".to_string(),
                    status: "missing".to_string(),
                    evidence: ev,
                    missing: miss,
                }
            }
        }
        "xim" => {
            let mut ev = Vec::new();
            let mut miss = Vec::new();
            if let Some(env) = env {
                if let Some(value) = env.get("XMODIFIERS").filter(|v| !v.trim().is_empty()) {
                    ev.push(format!("XMODIFIERS={value}"));
                }
            }
            let xim_env_ok = env_has(env.unwrap_or(&BTreeMap::new()), "XMODIFIERS", "@im=fcitx");
            if !xim_env_ok {
                miss.push("XMODIFIERS=@im=fcitx not set in target env".to_string());
            }
            if !locale_info.locale_valid {
                let loc = locale_info
                    .target_lc_ctype
                    .as_deref()
                    .or(locale_info.target_lang.as_deref())
                    .unwrap_or("C");
                miss.push(format!(
                    "locale '{loc}' is C/POSIX or not in locale -a; XIM may not activate"
                ));
            }
            if let Some(item) = loaded(&["im-xim", "libx11", "libxim"]) {
                ev.push(item);
                NamedPathCheck {
                    name: "xim".to_string(),
                    status: if xim_env_ok && locale_info.locale_valid {
                        "confirmed"
                    } else {
                        "configured"
                    }
                    .to_string(),
                    evidence: ev,
                    missing: miss,
                }
            } else if let Some(item) = available(&["im-xim"]) {
                ev.push(format!("available_on_disk={item}"));
                let locale_match =
                    check_immodule_locale("gtk_module", "xim", immodule_cache, locale_info);
                if !locale_match.is_empty() {
                    ev.push(locale_match);
                }
                NamedPathCheck {
                    name: "xim".to_string(),
                    status: if xim_env_ok && locale_info.locale_valid {
                        "configured"
                    } else {
                        "missing"
                    }
                    .to_string(),
                    evidence: ev,
                    missing: miss,
                }
            } else {
                miss.push("im-xim.so not found on disk".to_string());
                NamedPathCheck {
                    name: "xim".to_string(),
                    status: "missing".to_string(),
                    evidence: ev,
                    missing: miss,
                }
            }
        }
        _ => NamedPathCheck {
            name: path_name.to_string(),
            status: "unknown".to_string(),
            evidence: vec![],
            missing: vec!["unknown path name".to_string()],
        },
    }
}

fn check_immodule_locale(
    _path_name: &str,
    module_name: &str,
    immodule_cache: &[ImmoduleCacheEntry],
    locale_info: &LocaleInfo,
) -> String {
    let target_locale = locale_info
        .target_lc_ctype
        .as_deref()
        .or(locale_info.target_lang.as_deref())
        .unwrap_or("C");
    let locale_prefix = target_locale
        .split(|c: char| c == '.' || c == '_')
        .next()
        .unwrap_or("");
    let locale_lang = locale_prefix.split('_').next().unwrap_or("");

    for entry in immodule_cache {
        if !entry.module_name.contains(module_name) {
            continue;
        }
        let locales = &entry.locales;
        if locales.contains('*') {
            return format!(
                "immodule_cache: {} matches any locale (*)",
                entry.module_name
            );
        }
        let matches = locales.split(':').any(|loc| {
            loc == locale_prefix
                || loc == target_locale
                || loc == locale_lang
                || (loc.len() == 2 && locale_lang == loc)
        });
        return if matches {
            format!(
                "immodule_cache: {} locale '{}' matches target '{}'",
                entry.module_name, locales, target_locale
            )
        } else {
            format!(
                "immodule_cache: {} locale '{}' does NOT match target '{}'",
                entry.module_name, locales, target_locale
            )
        };
    }
    String::new()
}

