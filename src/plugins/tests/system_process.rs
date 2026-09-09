use crate::plugins::host::SaiPluginHost;
use sai_plugin_runtime::host::{PluginHost, ProcessRequest, SystemContext};
use sai_plugin_runtime::{Capabilities, InvocationContext, PluginPackage, PluginRuntime};
use serde_json::json;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// 【进程宿主测试】【测试进程】用当前测试二进制执行独立样本，不依赖平台 shell 或桌面应用。
/// @param root 临时目录；fixture 为精确样本名称；environment 为继承变量
/// @returns 可信目录、已授权模板和执行请求
fn setup(
    root: &Path,
    fixture: &str,
    environment: &[&str],
) -> (SystemContext, Capabilities, ProcessRequest) {
    std::fs::write(root.join("plugin-process-fixture"), "fixture").unwrap();
    let capabilities: Capabilities = serde_json::from_value(json!({"system":{
        "environment":environment, "processes":{"fixture":{
            "program":std::env::current_exe().unwrap().to_string_lossy(),
            "args":["--exact",format!("plugins::tests::system_process::{fixture}"),"--ignored","--nocapture"],
            "read_only":true
        }}
    }})).unwrap();
    let context = SystemContext {
        workdir: root.to_string_lossy().into_owned(),
        allow_writes: false,
    };
    let request = ProcessRequest {
        template: "fixture".into(),
        parameters: json!({}),
        timeout_ms: 3000,
        max_stdout_bytes: 64 * 1024,
        max_stderr_bytes: 64 * 1024,
    };
    (context, capabilities, request)
}

/// 【进程宿主测试】【管道限制】同时排空两路大输出，保存大小与截断标记符合请求。
#[tokio::test]
async fn process_output_is_bounded_and_drains_both_streams() {
    let root = tempfile::tempdir().unwrap();
    let (context, capabilities, mut request) = setup(root.path(), "fixture_output", &[]);
    request.max_stdout_bytes = 32;
    request.max_stderr_bytes = 16;
    let output = tokio::time::timeout(
        Duration::from_secs(5),
        SaiPluginHost.process(request, context, capabilities),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(output.status, Some(0));
    assert_eq!(output.stdout.len(), 32);
    assert_eq!(output.stderr.len(), 16);
    assert!(output.stdout_truncated && output.stderr_truncated);
}

/// 【进程宿主测试】【环境隔离】只继承授权变量，工作目录使用宿主上下文。
#[tokio::test]
async fn process_environment_contains_only_explicit_grants() {
    for environment in [vec![], vec!["PATH"]] {
        let root = tempfile::tempdir().unwrap();
        let (context, capabilities, request) =
            setup(root.path(), "fixture_environment", &environment);
        let output = SaiPluginHost
            .process(request, context, capabilities)
            .await
            .unwrap();
        assert_eq!(output.status, Some(0), "{}", output.stderr);
        assert!(
            output
                .stdout
                .contains(&format!("path={}", !environment.is_empty())),
            "{}",
            output.stdout
        );
        assert!(output.stdout.contains("home=false"));
        assert!(output.stdout.contains("cwd_has_fixture=true"));
    }
}

/// 【进程宿主测试】【执行属性】Unix 普通文本没有执行权限时，在创建进程前明确拒绝。
#[cfg(unix)]
#[tokio::test]
async fn non_executable_program_files_are_rejected() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("program");
    std::fs::write(&file, "text").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
    let (context, mut capabilities, request) = setup(root.path(), "fixture_output", &[]);
    capabilities
        .system
        .processes
        .get_mut("fixture")
        .unwrap()
        .program = file.to_string_lossy().into_owned();
    let error = SaiPluginHost
        .process(request, context, capabilities)
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("executable regular file"));
}

/// 【进程宿主测试】【批处理边界】Windows 不隐式用 cmd 执行模板中的批处理文件。
#[cfg(windows)]
#[tokio::test]
async fn windows_batch_programs_require_an_explicit_interpreter_template() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("program.CMD");
    std::fs::write(&file, "exit /b 0").unwrap();
    let (context, mut capabilities, request) = setup(root.path(), "fixture_output", &[]);
    capabilities
        .system
        .processes
        .get_mut("fixture")
        .unwrap()
        .program = file.to_string_lossy().into_owned();
    let error = SaiPluginHost
        .process(request, context, capabilities)
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("explicit interpreter"));
}

/// 【进程宿主测试】【普通结束】组长先结束但后代仍持有输出管道时，调用必须清理后代并返回。
#[tokio::test]
async fn completed_leaders_do_not_leave_descendants_or_open_pipes() {
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("exit-leader"), "exit").unwrap();
    let (context, capabilities, request) = setup(root.path(), "fixture_descendants", &[]);
    let output = tokio::time::timeout(
        Duration::from_secs(5),
        SaiPluginHost.process(request, context, capabilities),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(output.status, Some(0), "{}", output.stderr);
    let descendant = read_pid(&root.path().join("descendant.pid"));
    wait_stopped(descendant).await;
}

/// 【进程宿主测试】【运行时回收】真实 Lua 调用的超时和外部取消都必须终止组长与后代。
#[tokio::test]
async fn runtime_timeout_and_cancellation_reclaim_the_real_process_tree() {
    for cancelled in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let (context, capabilities, _) = setup(root.path(), "fixture_descendants", &[]);
        let manifest=sai_plugin_runtime::PluginManifest::parse(&json!({
            "api_version":1,"id":"process-test","version":"1.0.0","name":"Process test",
            "description":"Process lifetime contract","entry":"init.lua","capabilities":capabilities,
        }).to_string()).unwrap();
        let source = r#"
            sai.register_tool({name='run',description='Process lifetime',parameters={type='object'},execute=function(args)
                return sai.process.output('fixture',{}, {timeout_ms=args.timeout})
            end})
        "#;
        let package = PluginPackage::new(
            manifest,
            BTreeMap::from([("init.lua".into(), source.into())]),
        )
        .unwrap();
        let plugin = Arc::new(
            PluginRuntime::load(package, json!({}), capabilities, Arc::new(SaiPluginHost)).unwrap(),
        );
        let worker = plugin.clone();
        let call = tokio::spawn(async move {
            worker
                .call_tool(
                    "run",
                    json!({"timeout":if cancelled {10000} else {5000}}),
                    InvocationContext {
                        workdir: context.workdir,
                        ..Default::default()
                    },
                )
                .await
        });
        wait_file(&root.path().join("leader.pid")).await;
        wait_file(&root.path().join("descendant.pid")).await;
        let leader = read_pid(&root.path().join("leader.pid"));
        let descendant = read_pid(&root.path().join("descendant.pid"));
        if cancelled {
            call.abort();
            assert!(call.await.unwrap_err().is_cancelled());
        } else {
            let output = tokio::time::timeout(Duration::from_secs(10), call)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&output).unwrap()["timed_out"],
                true
            );
        }
        wait_stopped(leader).await;
        wait_stopped(descendant).await;
    }
}

/// 【进程宿主测试】【等待样本】等到测试进程写入完整 PID，不用固定启动延迟推断进程状态。
/// @param path PID 文件
/// @returns 无；超时则测试失败
pub(super) async fn wait_file(path: &Path) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if std::fs::read_to_string(path)
                .ok()
                .is_some_and(|value| value.parse::<u32>().is_ok())
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("test process did not create its pid marker");
}

/// 【进程宿主测试】【进程标识】读取样本明确写入的 PID，避免按名称影响其他进程。
/// @param path 当前临时目录内的标识文件
/// @returns 测试进程 PID
pub(super) fn read_pid(path: &Path) -> u32 {
    std::fs::read_to_string(path).unwrap().parse().unwrap()
}

/// 【进程宿主测试】【终止确认】验证真实进程已停止，Linux 僵尸仅代表等待父进程回收。
/// @param pid 当前样本的 PID
/// @returns 无；进程仍活动则测试失败
pub(super) async fn wait_stopped(pid: u32) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while process_running(pid) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("plugin process survived its owning invocation");
}

/// 【进程宿主测试】【平台状态】只读取指定测试进程的状态，不发送终止信号。
/// @param pid 测试进程 PID
/// @returns 是否仍在运行
fn process_running(pid: u32) -> bool {
    #[cfg(target_os = "linux")]
    {
        return std::fs::read_to_string(format!("/proc/{pid}/stat"))
            .ok()
            .and_then(|text| {
                text.rsplit_once(") ")
                    .map(|(_, state)| !state.starts_with(['Z', 'X']))
            })
            .unwrap_or(false);
    }
    #[cfg(all(unix, not(target_os = "linux")))]
    {
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{CloseHandle, WAIT_TIMEOUT};
        use windows_sys::Win32::System::Threading::{
            OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE,
        };
        let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
        if handle.is_null() {
            return false;
        }
        let running = unsafe { WaitForSingleObject(handle, 0) } == WAIT_TIMEOUT;
        unsafe {
            CloseHandle(handle);
        }
        running
    }
}

/// 【进程宿主测试】【大输出样本】仅在测试创建的隔离目录内产生有界输出。
#[test]
#[ignore = "仅由进程宿主测试在隔离目录启动"]
fn fixture_output() {
    use std::io::Write;
    if !Path::new("plugin-process-fixture").is_file() {
        return;
    }
    let block = vec![b'x'; 128 * 1024];
    std::io::stdout().write_all(&block).unwrap();
    std::io::stderr().write_all(&block).unwrap();
}

/// 【进程宿主测试】【环境样本】只输出变量是否存在，避免暴露环境内容。
#[test]
#[ignore = "仅由进程宿主测试在隔离目录启动"]
fn fixture_environment() {
    if !Path::new("plugin-process-fixture").is_file() {
        return;
    }
    println!(
        "path={} home={} cwd_has_fixture=true",
        std::env::var_os("PATH").is_some(),
        std::env::var_os("HOME").is_some()
    );
}

/// 【进程宿主测试】【后代样本】启动同一测试二进制中的等待进程，模拟组长与后代不同生命周期。
#[test]
#[ignore = "仅由进程宿主测试在隔离目录启动"]
fn fixture_descendants() {
    if !Path::new("plugin-process-fixture").is_file() {
        return;
    }
    std::fs::write("leader.pid", std::process::id().to_string()).unwrap();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "plugins::tests::system_process::fixture_wait",
            "--ignored",
            "--nocapture",
        ])
        .spawn()
        .unwrap();
    for _ in 0..400 {
        if std::fs::read_to_string("descendant.pid")
            .ok()
            .is_some_and(|value| value.parse::<u32>().is_ok())
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    if PathBuf::from("exit-leader").exists() {
        return;
    }
    let _ = child.wait();
}

/// 【进程宿主测试】【等待样本】公开当前 PID 后等待宿主回收，设置兜底退出避免测试失败留下常驻进程。
#[test]
#[ignore = "仅由进程宿主测试在隔离目录启动"]
fn fixture_wait() {
    if !Path::new("plugin-process-fixture").is_file() {
        return;
    }
    std::fs::write("descendant.pid", std::process::id().to_string()).unwrap();
    std::thread::sleep(Duration::from_secs(20));
}
