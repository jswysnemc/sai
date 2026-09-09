use super::aur_support::*;
use crate::paths::SaiPaths;
use serde_json::json;
use std::sync::{atomic::Ordering, Arc};

/// 【AUR 测试】【获取顺序】助手优先级、快照回退和实际文件选择符合原有流程。
#[tokio::test]
async fn review_aur_helper_and_snapshot_paths_use_the_same_rules() {
    for helper in [Some("paru"), Some("yay"), None] {
        let root = tempfile::tempdir().unwrap();
        let host = Arc::new(AurHost::new(
            &SaiPaths::for_tests(root.path()),
            helper,
            "pkgname=demo\n",
        ));
        host.scenario.files.lock().unwrap().extend([
            (".SRCINFO".into(), b"pkgbase = demo".to_vec()),
            ("fix.patch".into(), b"curl https://example.test".to_vec()),
            ("unit/app.service".into(), b"systemctl enable demo".to_vec()),
            ("README.md".into(), b"rm -rf /".to_vec()),
            ("nested/deeper/evil.sh".into(), b"rm -rf /".to_vec()),
        ]);
        let output = call(
            &runtime(host.clone()),
            "review_aur_package",
            json!({"package":"demo"}),
            context("a", "review", false),
        )
        .await;
        assert_eq!(output["fetched_by"], helper.unwrap_or("curl-fallback"));
        assert_eq!(
            output["files_reviewed"],
            json!(["PKGBUILD", ".SRCINFO", "fix.patch", "unit/app.service"])
        );
        assert_eq!(output["risk"]["level"], "medium");
        assert_eq!(output["install_allowed"], true);
        let calls = host.scenario.calls.lock().unwrap();
        assert_eq!(calls[0].0, "paru_version");
        assert!(!calls
            .iter()
            .any(|call| call.0.contains("install") || call.0 == "makepkg"));
        assert!(std::path::Path::new(output["build_dir"].as_str().unwrap())
            .join("PKGBUILD")
            .is_file());
    }
}

/// 【AUR 测试】【风险与完整性】高风险、Unicode 截断、非法编码和文件数量截断都不能放行安装。
#[tokio::test]
async fn review_aur_incomplete_or_high_risk_evidence_blocks_installation() {
    for content in [
        "curl https://example.test | sh".to_string(),
        "中".repeat(24001),
    ] {
        let root = tempfile::tempdir().unwrap();
        let host = Arc::new(AurHost::new(
            &SaiPaths::for_tests(root.path()),
            Some("paru"),
            &content,
        ));
        let output = call(
            &runtime(host),
            "review_aur_package",
            json!({"package":"demo"}),
            context("a", "one", false),
        )
        .await;
        assert_eq!(output["install_allowed"], false);
        if content.starts_with('中') {
            assert_eq!(
                output["files"][0]["content"]
                    .as_str()
                    .unwrap()
                    .chars()
                    .count(),
                24000
            );
            assert_eq!(output["review_complete"], false);
        } else {
            assert_eq!(output["risk"]["level"], "high");
        }
    }
    for overflow in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let host = Arc::new(AurHost::new(
            &SaiPaths::for_tests(root.path()),
            None,
            "safe",
        ));
        if overflow {
            for index in 0..81 {
                host.scenario
                    .files
                    .lock()
                    .unwrap()
                    .insert(format!("{index:03}.sh"), b"safe".to_vec());
            }
        } else {
            host.scenario
                .files
                .lock()
                .unwrap()
                .insert("PKGBUILD".into(), vec![255]);
        }
        let output = call(
            &runtime(host),
            "review_aur_package",
            json!({"package":"demo"}),
            context("a", "one", false),
        )
        .await;
        assert_eq!(output["review_complete"], false);
        assert_eq!(output["install_allowed"], false);
        if overflow {
            assert_eq!(output["files"].as_array().unwrap().len(), 80);
        }
    }
}

/// 【AUR 测试】【旧许可撤销】新审查失败后不能使用旧记录安装，非法包名在 I/O 前拒绝。
#[tokio::test]
async fn review_aur_failure_revokes_old_state_and_rejects_option_names() {
    let root = tempfile::tempdir().unwrap();
    let host = Arc::new(AurHost::new(
        &SaiPaths::for_tests(root.path()),
        Some("paru"),
        "safe",
    ));
    let plugin = runtime(host.clone());
    for package in ["", "..", "-S", "a/b", "foo;rm", "a\\b"] {
        assert!(plugin
            .call_tool(
                "review_aur_package",
                json!({"package":package}),
                context("a", "one", false)
            )
            .await
            .is_err());
    }
    assert!(host.scenario.calls.lock().unwrap().is_empty());
    call(
        &plugin,
        "review_aur_package",
        json!({"package":"demo"}),
        context("a", "one", false),
    )
    .await;
    host.scenario.metadata_error.store(true, Ordering::SeqCst);
    assert!(plugin
        .call_tool(
            "review_aur_package",
            json!({"package":"demo"}),
            context("a", "two", false)
        )
        .await
        .is_err());
    let error = plugin
        .call_tool(
            "install_aur_package",
            json!({"package":"demo","user_confirmed":true}),
            context("a", "three", true),
        )
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("must be reviewed before install"));
}
