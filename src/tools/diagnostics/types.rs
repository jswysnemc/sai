use super::{ToolRegistry, ToolSpec};
use crate::config::{AppConfig, DiagnosticsPluginConfig};
use anyhow::{bail, Result};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

pub fn register(registry: &mut ToolRegistry, config: AppConfig) {
    registry.register(ToolSpec::new(
        "check_issue",
        "Collect read-only diagnostic evidence for a concrete local issue. This tool gathers facts only; it does not diagnose, rank root causes, or recommend fixes.",
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Original user issue. Used for auto area/target inference." },
                "area": { "type": "string", "enum": ["auto", "system", "app", "input_method", "display", "audio", "package", "gpu", "network", "storage"], "description": "Evidence collection area." },
                "target": { "type": "string", "description": "Optional app, process, command, package, or subsystem target." },
                "symptom": { "type": "string", "description": "Optional symptom label." },
                "depth": { "type": "string", "enum": ["quick", "normal", "full"], "description": "Probe depth." },
                "recent_minutes": { "type": "integer", "description": "Recent log window in minutes, clamped to 1..1440." },
                "platform": { "type": "string", "enum": ["auto", "linux", "macos"], "description": "Platform override. Prefer auto." },
                "allow_launch_probe": { "type": "boolean", "description": "For app/input_method evidence only: explicitly allow launching target to sample runtime facts. Defaults to false." },
                "launch_timeout_seconds": { "type": "integer", "description": "Seconds to wait after launch probe before sampling pids. Defaults to 3, max 15." }
            },
            "required": [],
            "additionalProperties": false
        }),
        move |args| {
            let config = config.clone();
            async move { check_issue(args, config.plugins.diagnostics.clone()).await }
        },
    ));
}

#[derive(Debug, Clone)]
struct CheckIssueArgs {
    query: Option<String>,
    area: Area,
    target: Option<String>,
    symptom: Option<String>,
    depth: Depth,
    recent_minutes: u64,
    platform: PlatformArg,
    allow_launch_probe: bool,
    launch_timeout_seconds: u64,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum Area {
    System,
    App,
    InputMethod,
    Display,
    Audio,
    Package,
    Gpu,
    Network,
    Storage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Depth {
    Quick,
    Normal,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlatformArg {
    Auto,
    Linux,
    Macos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Platform {
    Linux,
    Macos,
    Unsupported,
}

#[derive(Debug, Serialize)]
struct EvidenceReport {
    ok: bool,
    kind: &'static str,
    platform: Platform,
    query: Option<String>,
    area: Area,
    target: Option<String>,
    symptom: Option<String>,
    depth: Depth,
    facts: BTreeMap<String, Value>,
    checks: Vec<Check>,
    logs: Vec<LogExcerpt>,
    missing_evidence: Vec<String>,
    safety_notes: Vec<String>,
    recommended_next_probes: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Check {
    id: String,
    status: CheckStatus,
    detail: String,
    evidence: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum CheckStatus {
    Ok,
    Warn,
    Error,
    Unknown,
}

#[derive(Debug, Serialize)]
struct LogExcerpt {
    source: String,
    message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum InputToolkit {
    ElectronChromium,
    ElectronX11,
    ElectronWayland,
    Gtk,
    Qt,
    Sdl,
    Java,
    X11Legacy,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum DisplayMode {
    X11,
    XWayland,
    WaylandNative,
    Unknown,
}

#[derive(Debug, Serialize)]
struct InputMethodProfile {
    toolkit: InputToolkit,
    display_mode: DisplayMode,
    runtime_observed: bool,
    command_line: Option<String>,
    desktop_exec: Option<String>,
    target_env: Option<BTreeMap<String, String>>,
    loaded_input_modules: Vec<String>,
    available_input_modules: Vec<String>,
    immodule_cache: Vec<ImmoduleCacheEntry>,
    wayland_protocol: WaylandProtocolInfo,
    locale_info: LocaleInfo,
    path_status: InputMethodPathStatus,
}

#[derive(Debug, Clone, Serialize)]
struct ImmoduleCacheEntry {
    so_path: String,
    module_name: String,
    locales: String,
}

#[derive(Debug, Serialize)]
struct WaylandProtocolInfo {
    compositor_supports_text_input_v3: bool,
    fcitx5_wayland_frontend_loaded: bool,
    wayland_info_available: bool,
}

#[derive(Debug, Serialize)]
struct LocaleInfo {
    target_lang: Option<String>,
    target_lc_ctype: Option<String>,
    available_locales: Vec<String>,
    locale_valid: bool,
}

#[derive(Debug, Serialize)]
struct InputMethodPathStatus {
    paths: Vec<NamedPathCheck>,
    overall: String,
}

#[derive(Debug, Serialize)]
struct NamedPathCheck {
    name: String,
    status: String,
    evidence: Vec<String>,
    missing: Vec<String>,
}

#[derive(Debug)]
struct ProbeOutput {
    status: Option<i32>,
    stdout: String,
    stderr: String,
    timed_out: bool,
}

