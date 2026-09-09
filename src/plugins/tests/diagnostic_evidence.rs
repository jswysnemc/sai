#[cfg(unix)]
use super::diagnostic_support::parsed;
use super::diagnostic_support::DiagnosticHost;
use super::support::{call_json, runtime};
#[cfg(unix)]
use anyhow::Result;
#[cfg(unix)]
use async_trait::async_trait;
#[cfg(unix)]
use sai_plugin_runtime::host::{HostTool, InvocationServices, ModelRequest, ModelResponse};
use sai_plugin_runtime::InvocationContext;
#[cfg(unix)]
use sai_plugin_runtime::ToolAccess;
use serde_json::{json, Value};
#[cfg(unix)]
use std::sync::atomic::Ordering;
use std::sync::Arc;
#[cfg(unix)]
use std::sync::Mutex;

/// 【诊断迁移测试】【九类采集】真实 Lua 工具按领域选择模板，保留报告字段，不能隐式执行应用版本。
#[tokio::test]
async fn all_linux_areas_collect_evidence_without_executing_the_target() {
    for (area, expected) in [
        ("system", "command-path"),
        ("app", "package-owner"),
        ("input_method", "wayland-info"),
        ("display", "systemd-user"),
        ("audio", "audio-status"),
        ("package", "recent-logs"),
        ("gpu", "pci"),
        ("network", "network-addresses"),
        ("storage", "disk-usage"),
    ] {
        let host = Arc::new(DiagnosticHost::default());
        let plugin = runtime("diagnostic-evidence", host.clone());
        let output = call_json(
            &plugin,
            "check_issue",
            json!({"platform":"linux","area":area,"target":"example"}),
        )
        .await;
        assert_eq!(output["kind"], "diagnostic_evidence");
        assert_eq!(output["ok"], true);
        assert_eq!(output["facts"]["os.pretty_name"], "Fixture Linux");
        assert_eq!(output["query"], Value::Null);
        assert_eq!(output["symptom"], Value::Null);
        for field in [
            "checks",
            "logs",
            "missing_evidence",
            "safety_notes",
            "recommended_next_probes",
        ] {
            assert!(output[field].is_array(), "{area}: {field}");
        }
        let calls = host.calls.lock().unwrap();
        assert!(calls.iter().any(|call| call.template == expected), "{area}");
        assert!(!calls.iter().any(|call| call.template == "app-version"));
        if area == "network" {
            assert!(!output.to_string().contains("192.168.50.1"));
        }
        if area == "package" {
            assert_eq!(output["facts"]["package.pacman_db_lock_exists"], true);
        }
    }
}

/// 【诊断迁移测试】【输入法报告】组合真实模块解释器与固定宿主，保留路径和运行状态并过滤无关环境。
#[tokio::test]
async fn input_method_profiles_combine_runtime_modules_protocols_and_locales() {
    let host = Arc::new(DiagnosticHost::default());
    let plugin = runtime("diagnostic-evidence", host.clone());
    let output = call_json(
        &plugin,
        "check_issue",
        json!({"area":"input_method","target":"example","platform":"linux"}),
    )
    .await;
    let profile = &output["facts"]["input_method.profile"];
    assert_eq!(profile["toolkit"], "electron_wayland");
    assert_eq!(profile["display_mode"], "wayland_native");
    assert_eq!(profile["runtime_observed"], true);
    assert_eq!(profile["locale_info"]["locale_valid"], true);
    assert_eq!(
        profile["wayland_protocol"]["fcitx5_wayland_frontend_loaded"],
        true
    );
    assert_eq!(profile["path_status"]["overall"], "path_evidence_complete");
    assert!(!output.to_string().contains("must-not-appear"));
    assert!(!output.to_string().contains("SECRET"));
    assert!(output["facts"]["input_method.package_probe"]
        .as_str()
        .unwrap()
        .contains("fixture-app"));
    assert!(host
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|call| call.template == "package-files"));
}

/// 【诊断迁移测试】【缺失证据】单项超时或文件缺失不抹去已采集事实，快速模式不访问日志。
#[tokio::test]
async fn partial_failures_remain_visible_and_quick_mode_skips_journal_reads() {
    let mut host = DiagnosticHost::default();
    host.timeouts.insert("wayland-info".into());
    host.files.remove("/proc/321/environ");
    let host = Arc::new(host);
    let plugin = runtime("diagnostic-evidence", host.clone());
    let output = call_json(
        &plugin,
        "check_issue",
        json!({"area":"input_method","target":"example","platform":"linux","depth":"quick"}),
    )
    .await;
    assert_eq!(output["facts"]["os.pretty_name"], "Fixture Linux");
    assert_eq!(
        output["facts"]["input_method.profile"]["target_env"],
        Value::Null
    );
    assert!(output["missing_evidence"].to_string().contains("timed out"));
    assert!(output["missing_evidence"].to_string().contains("environ"));
    assert!(!host
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|call| call.template == "recent-logs"));
}

/// 【诊断迁移测试】【旧参数边界】旧启动选项提供可执行迁移说明，并在任何系统采集前拒绝。
#[tokio::test]
async fn legacy_launch_requests_are_rejected_before_any_system_probe() {
    let host = Arc::new(DiagnosticHost::default());
    let plugin = runtime("diagnostic-evidence", host.clone());
    let error = plugin
        .call_tool(
            "check_issue",
            json!({"target":"example","allow_launch_probe":true}),
            InvocationContext::default(),
        )
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("diagnostic_app_probe with probe=launch"));
    assert!(host.calls.lock().unwrap().is_empty());
    assert!(host.reads.lock().unwrap().is_empty());
}

/// 【诊断迁移测试】【平台兼容】macOS 保留基础系统、命令和进程证据，不执行 Linux 模板。
#[tokio::test]
async fn macos_keeps_its_original_basic_evidence_contract() {
    let host = Arc::new(DiagnosticHost::default());
    let plugin = runtime("diagnostic-evidence", host.clone());
    let output = call_json(
        &plugin,
        "check_issue",
        json!({"platform":"macos","target":"example"}),
    )
    .await;
    assert_eq!(output["platform"], "macos");
    assert_eq!(output["facts"]["env.lang"], "zh_CN.UTF-8");
    assert_eq!(
        host.calls
            .lock()
            .unwrap()
            .iter()
            .map(|call| call.template.as_str())
            .collect::<Vec<_>>(),
        vec!["macos-info", "command-path", "processes"]
    );
}

/// 【诊断迁移测试】【不支持平台】Windows 自动模式返回明确状态，不进行任何系统访问。
#[cfg(windows)]
#[tokio::test]
async fn unsupported_windows_auto_mode_performs_no_system_io() {
    let host = Arc::new(DiagnosticHost::default());
    let plugin = runtime("diagnostic-evidence", host.clone());
    let output = call_json(&plugin, "check_issue", json!({"area":"system"})).await;
    assert_eq!(output["ok"], false);
    assert_eq!(output["platform"], "unsupported");
    assert!(host.calls.lock().unwrap().is_empty());
    assert!(host.reads.lock().unwrap().is_empty());
}

/// 【诊断迁移测试】【版本权限】版本采集保留在写入入口，缺少可信写入权限时不能执行模板。
#[cfg(unix)]
#[tokio::test]
async fn explicit_version_execution_requires_write_permission() {
    let host = Arc::new(DiagnosticHost::default());
    let plugin = runtime("diagnostic-evidence", host.clone());
    let args = json!({"target":"example","probe":"version","platform":"linux","depth":"quick"});
    let denied = call_json(&plugin, "diagnostic_app_probe", args.clone()).await;
    assert_eq!(denied["ok"], false);
    assert!(!host
        .calls
        .lock()
        .unwrap()
        .iter()
        .any(|call| call.template == "app-version"));
    let output = parsed(
        plugin
            .call_tool(
                "diagnostic_app_probe",
                args,
                InvocationContext {
                    allow_writes: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap(),
    );
    assert_eq!(output["ok"], true);
    assert!(output["logs"].to_string().contains("example 1.2.3"));
    assert_eq!(
        host.calls
            .lock()
            .unwrap()
            .iter()
            .filter(|call| call.template == "app-version")
            .count(),
        1
    );
}

#[cfg(unix)]
struct LaunchServices {
    host: Arc<DiagnosticHost>,
    calls: Mutex<Vec<Value>>,
}

#[cfg(unix)]
#[async_trait]
impl InvocationServices for LaunchServices {
    /// 【诊断启动测试】【写入目录】提供唯一写入工具，让真实运行时继续检查权限。
    /// @returns 固定工具目录
    fn tools(&self) -> Result<Vec<HostTool>> {
        Ok(vec![HostTool {
            name: "run_command".into(),
            display_name: "run_command".into(),
            description: "Fixture launch".into(),
            parameters: json!({"type":"object"}),
            access: ToolAccess::Writes,
        }])
    }
    /// 【诊断启动测试】【启动替身】记录参数并改变进程证据，不启动真实应用。
    /// @param name 工具名；arguments 为参数 JSON
    /// @returns 受管后台任务样本
    async fn call_tool(&self, name: &str, arguments: &str) -> Result<String> {
        assert_eq!(name, "run_command");
        self.calls
            .lock()
            .unwrap()
            .push(serde_json::from_str(arguments)?);
        self.host.running.store(true, Ordering::SeqCst);
        Ok(json!({"mode":"background","task_id":"fixture-task","task":{"pid":320}}).to_string())
    }
    /// 【诊断启动测试】【禁止模型】应用采样不需要模型服务。
    /// @param request 模型请求；max_bytes 为上限
    /// @returns 明确错误
    async fn complete(&self, _request: ModelRequest, _max_bytes: usize) -> Result<ModelResponse> {
        anyhow::bail!("no model fixture")
    }
}

/// 【诊断迁移测试】【显式启动】权限通过后才调用受管命令，保留 PID 采样与任务管理入口。
#[cfg(unix)]
#[tokio::test]
async fn explicit_launch_retains_process_evidence_and_a_managed_task_handle() {
    let host = Arc::new(DiagnosticHost::default());
    host.running.store(false, Ordering::SeqCst);
    let services = Arc::new(LaunchServices {
        host: host.clone(),
        calls: Mutex::default(),
    });
    let plugin = runtime("diagnostic-evidence", host.clone());
    let args = json!({"probe":"launch","target":"example","area":"input_method","platform":"linux","depth":"quick","launch_timeout_seconds":2});
    assert!(plugin
        .call_tool(
            "diagnostic_app_probe",
            args.clone(),
            InvocationContext {
                services: Some(services.clone()),
                ..Default::default()
            }
        )
        .await
        .is_err());
    assert!(services.calls.lock().unwrap().is_empty());
    let output = parsed(
        plugin
            .call_tool(
                "diagnostic_app_probe",
                args,
                InvocationContext {
                    allow_writes: true,
                    workdir: "/trusted".into(),
                    services: Some(services.clone()),
                    ..Default::default()
                },
            )
            .await
            .unwrap(),
    );
    assert_eq!(
        output["facts"]["launch_probe"]["managed_task_id"],
        "fixture-task"
    );
    assert_eq!(output["facts"]["launch_probe"]["pids_before"], json!([]));
    assert_eq!(output["facts"]["launch_probe"]["new_pids"], json!([321]));
    assert_eq!(
        output["facts"]["input_method.profile"]["runtime_observed"],
        true
    );
    let calls = services.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0],
        json!({"command":"exec example","timeout_seconds":2,"cwd":"/trusted","label":"Diagnostic launch: example"})
    );
}

/// 【诊断迁移测试】【名称限制】写入入口也不能把选项、路径或 shell 片段解释为目标命令。
#[tokio::test]
async fn invalid_launch_targets_never_reach_process_or_tool_services() {
    let host = Arc::new(DiagnosticHost::default());
    let plugin = runtime("diagnostic-evidence", host.clone());
    for target in [
        "-option",
        "../example",
        "a;touch marker",
        "A=value",
        "/usr/bin/example",
        "a b",
    ] {
        assert!(plugin
            .call_tool(
                "diagnostic_app_probe",
                json!({"probe":"launch","target":target}),
                InvocationContext {
                    allow_writes: true,
                    ..Default::default()
                }
            )
            .await
            .is_err());
    }
    assert!(host.calls.lock().unwrap().is_empty());
}
