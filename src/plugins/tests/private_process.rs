use crate::{paths::SaiPaths, plugins::private::PrivatePluginHost};
#[cfg(unix)]
use sai_plugin_runtime::host::{PluginHost, ProcessRequest};
use sai_plugin_runtime::{Capabilities, InvocationContext, PluginPackage, PluginRuntime};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

/// 【私有进程测试】【真实进程生命周期】私有目录中的普通进程及后代在超时或取消后全部回收。
#[tokio::test]
async fn private_workspace_processes_and_directory_locks_end_with_the_callback() {
    for cancelled in [false, true] {
        let root = tempfile::tempdir().unwrap();
        // 【私有进程测试】【路径别名】1. Unix 固定使用符号链接根，覆盖临时目录别名与真实路径不同的情况
        #[cfg(unix)]
        let paths = {
            let actual = root.path().join("actual");
            let alias = root.path().join("alias");
            std::fs::create_dir(&actual).unwrap();
            std::os::unix::fs::symlink(&actual, &alias).unwrap();
            SaiPaths::for_tests(&alias)
        };
        #[cfg(not(unix))]
        let paths = SaiPaths::for_tests(root.path());
        let capabilities: Capabilities = serde_json::from_value(json!({"system":{
            "workspace":true,"processes":{"fixture":{
                "program":std::env::current_exe().unwrap().to_str().unwrap(),
                "args":["--exact","plugins::tests::system_process::fixture_descendants","--ignored","--nocapture"],
                "read_only":true,"workspace":true
            }}
        }})).unwrap();
        let source = r#"
            sai.register_tool({name="run",description="process",parameters={type="object"},execute=function(args, ctx)
                local work = sai.workspace.open("process")
                if args.open_only then return work:path() end
                ctx.progress(work:path())
                return work:process("fixture", {}, {timeout_ms=args.timeout, directory=args.directory})
            end})
        "#;
        let mut descriptor = super::support::descriptor("private-process", source);
        descriptor.package.manifest.capabilities = capabilities.clone();
        let sources = descriptor.package.sources().clone();
        let package = PluginPackage::new(descriptor.package.manifest, sources).unwrap();
        let plugin = Arc::new(
            PluginRuntime::load(
                package,
                json!({}),
                capabilities,
                Arc::new(PrivatePluginHost::new(&paths, "private-process")),
            )
            .unwrap(),
        );
        let (send, mut receive) = tokio::sync::mpsc::unbounded_channel();
        let subdirectory = if cancelled {
            format!("nested-{}", "x".repeat(160))
        } else {
            ".".into()
        };
        let fixture_directory = subdirectory.clone();
        let cache = paths.cache_dir.clone();
        let progress = Arc::new(move |message: String| {
            let path = std::path::PathBuf::from(message);
            let cache = dunce::canonicalize(&cache).unwrap();
            assert!(
                path.starts_with(&cache),
                "workspace {} is outside cache {}",
                path.display(),
                cache.display()
            );
            let process_path = path.join(&fixture_directory);
            std::fs::create_dir_all(&process_path).unwrap();
            std::fs::write(process_path.join("plugin-process-fixture"), "fixture").unwrap();
            send.send((path, process_path)).unwrap();
        });
        let worker = plugin.clone();
        let mut task = tokio::spawn(async move {
            worker
                .call_tool(
                    "run",
                    json!({"timeout":if cancelled {10000} else {5000}, "directory":subdirectory}),
                    InvocationContext {
                        progress: Some(progress),
                        ..Default::default()
                    },
                )
                .await
        });
        let (path, process_path) = tokio::time::timeout(Duration::from_secs(5), receive.recv())
            .await
            .unwrap()
            .unwrap();
        // 【私有进程测试】【启动结果】2. 启动提前失败时报告工具结果，不把底层错误掩盖成标记等待超时
        tokio::select! {
            biased;
            result = &mut task => panic!(
                "private process ended before pid markers in {}: {result:?}", path.display()
            ),
            _ = async {
                super::system_process::wait_file(&process_path.join("leader.pid")).await;
                super::system_process::wait_file(&process_path.join("descendant.pid")).await;
            } => {}
        }
        let leader = super::system_process::read_pid(&process_path.join("leader.pid"));
        let descendant = super::system_process::read_pid(&process_path.join("descendant.pid"));
        if cancelled {
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
        } else {
            let output = tokio::time::timeout(Duration::from_secs(10), task)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&output).unwrap()["timed_out"],
                true
            );
        }
        super::system_process::wait_stopped(leader).await;
        super::system_process::wait_stopped(descendant).await;
        let reopened = plugin
            .call_tool(
                "run",
                json!({"open_only":true}),
                InvocationContext::default(),
            )
            .await
            .unwrap();
        assert_eq!(std::path::PathBuf::from(reopened), path);
        assert!(!process_path.join("descendant.pid").exists());
    }
}

/// 【私有进程测试】【根目录替换】创建后替换目录为外部链接，不能重新确定可信根并执行进程。
#[cfg(unix)]
#[tokio::test]
async fn private_workspace_process_rejects_rebound_root() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let host = PrivatePluginHost::new(&paths, "private-process");
    let mut capabilities = Capabilities::default();
    capabilities.system.workspace = true;
    let workspace = host.workspace("root", "session", &capabilities).unwrap();
    let path = std::path::PathBuf::from(workspace.path());
    let outside = root.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::fs::rename(&path, path.with_file_name("original-root")).unwrap();
    std::os::unix::fs::symlink(&outside, &path).unwrap();
    let error = workspace
        .process(
            ProcessRequest {
                template: "must-not-run".into(),
                parameters: json!({}),
                timeout_ms: 1000,
                max_stdout_bytes: 64,
                max_stderr_bytes: 64,
            },
            ".".into(),
            capabilities,
            false,
        )
        .await
        .unwrap_err();
    assert!(
        error.to_string().contains("outside the private workspace"),
        "{error:#}"
    );
}
