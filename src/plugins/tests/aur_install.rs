use super::aur_support::*;
use crate::paths::SaiPaths;
use serde_json::json;
use std::sync::Arc;

/// 【AUR 测试】【确认边界】会话、操作、布尔确认及写入权限全部通过后才产生一次安装调用。
#[tokio::test]
async fn install_aur_requires_later_confirmation_and_consumes_review_once() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let host = Arc::new(AurHost::new(&paths, Some("paru"), "safe"));
    let plugin = runtime(host.clone());
    call(
        &plugin,
        "review_aur_package",
        json!({"package":"demo"}),
        context("a", "one", false),
    )
    .await;
    for (session, operation, confirmed, writes, expected) in [
        ("a", "one", true, true, "cannot run in the same turn"),
        ("b", "two", true, true, "must be reviewed"),
        ("a", "two", false, true, "explicit user confirmation"),
        ("a", "two", true, false, "read-only"),
    ] {
        let error = plugin
            .call_tool(
                "install_aur_package",
                json!({"package":"demo","user_confirmed":confirmed}),
                context(session, operation, writes),
            )
            .await
            .unwrap_err();
        assert!(format!("{error:#}").contains(expected), "{error:#}");
    }
    let output = call(
        &runtime(host.clone()),
        "install_aur_package",
        json!({"package":"demo","user_confirmed":true}),
        context("a", "two", true),
    )
    .await;
    assert_eq!(output["ok"], true);
    assert_eq!(output["review"]["user_confirmed_install"], true);
    assert_eq!(output["install_result"]["command"], "paru");
    assert!(plugin
        .call_tool(
            "install_aur_package",
            json!({"package":"demo","user_confirmed":true}),
            context("a", "three", true)
        )
        .await
        .is_err());
    assert_eq!(
        host.scenario
            .calls
            .lock()
            .unwrap()
            .iter()
            .filter(|call| call.0 == "paru_install")
            .count(),
        1
    );
    crate::plugins::clear_session_storage(&paths.state_dir, "a").unwrap();
    assert!(plugin
        .call_tool(
            "install_aur_package",
            json!({"package":"demo","user_confirmed":true}),
            context("a", "four", true)
        )
        .await
        .is_err());
}

/// 【AUR 测试】【安装编排】paru、yay 和 makepkg 回退使用原有 argv 及步骤超时。
#[tokio::test]
async fn install_aur_runs_helper_or_build_then_pacman() {
    for helper in [Some("paru"), Some("yay"), None] {
        let root = tempfile::tempdir().unwrap();
        let host = Arc::new(AurHost::new(
            &SaiPaths::for_tests(root.path()),
            helper,
            "safe",
        ));
        let plugin = runtime(host.clone());
        call(
            &plugin,
            "review_aur_package",
            json!({"package":"demo"}),
            context("a", "one", false),
        )
        .await;
        host.scenario.calls.lock().unwrap().clear();
        let output = call(
            &plugin,
            "install_aur_package",
            json!({"package":"demo","user_confirmed":true}),
            context("a", "two", true),
        )
        .await;
        assert_eq!(output["ok"], true);
        let calls = host.scenario.calls.lock().unwrap();
        let last = calls.last().unwrap();
        assert_eq!(last.3, 900000);
        if let Some(helper) = helper {
            assert_eq!(last.0, format!("{helper}_install"));
        } else {
            assert_eq!(calls[calls.len() - 2].0, "makepkg");
            assert_eq!(calls[calls.len() - 2].3, 1800000);
            assert_eq!(last.0, "pacman_install");
            assert_eq!(last.1["archive"], "./demo-1-1-any.pkg.tar.zst");
            assert_eq!(last.2, "snapshot/demo");
        }
    }
}

/// 【AUR 测试】【失败停止】构建失败或超时后不调用 pacman，安装失败不重复消费记录。
#[tokio::test]
async fn install_aur_build_failure_and_timeout_stop_the_chain() {
    for timed_out in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let host = Arc::new(AurHost::new(
            &SaiPaths::for_tests(root.path()),
            None,
            "safe",
        ));
        let mut failed = success();
        failed.status = Some(7);
        failed.timed_out = timed_out;
        host.scenario
            .outcomes
            .lock()
            .unwrap()
            .insert("makepkg".into(), failed);
        let plugin = runtime(host.clone());
        call(
            &plugin,
            "review_aur_package",
            json!({"package":"demo"}),
            context("a", "one", false),
        )
        .await;
        let result = plugin
            .call_tool(
                "install_aur_package",
                json!({"package":"demo","user_confirmed":true}),
                context("a", "two", true),
            )
            .await;
        if timed_out {
            assert!(format!("{:#}", result.unwrap_err()).contains("makepkg timed out after 1800s"));
        } else {
            let output: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
            assert_eq!(output["ok"], false);
        }
        assert!(!host
            .scenario
            .calls
            .lock()
            .unwrap()
            .iter()
            .any(|call| call.0 == "pacman_install"));
    }
}

/// 【AUR 测试】【平台边界】实际平台常量不受测试场景替换影响，非 Linux 在任何进程前拒绝安装。
#[cfg(not(target_os = "linux"))]
#[tokio::test]
async fn install_aur_rejects_the_actual_non_linux_platform() {
    let root = tempfile::tempdir().unwrap();
    let host = Arc::new(AurHost::new(
        &SaiPaths::for_tests(root.path()),
        None,
        "safe",
    ));
    let package = super::query_support::package("package-advisor");
    let grants = package.manifest.capabilities.clone();
    let plugin =
        sai_plugin_runtime::PluginRuntime::load(package, json!({}), grants, host.clone()).unwrap();
    let error = plugin
        .call_tool(
            "install_aur_package",
            json!({"package":"demo","user_confirmed":true}),
            context("a", "one", true),
        )
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("only supported on Linux"));
    assert!(host.scenario.calls.lock().unwrap().is_empty());
}
