use super::*;
use crate::cli::mode::{cli_mode_override, resolve_agent_mode};
use crate::cli::{Cli, PermissionSurface};
use crate::config::CodeMode;
use clap::Parser;

/// 【工具方式】【启动继承】保存三种工具方式，后续终端和命令行启动沿用配置且不改权限。
/// 参数: 无；返回: 无。
#[test]
fn saved_selection_survives_startup_without_changing_other_settings() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    let mut config = AppConfig::default();
    config.codemode.inline_budget = 1234;
    config.codemode.timeout_ms = 4321;
    config.save(&paths).unwrap();
    let permission = serde_json::to_value(&config.permission).unwrap();
    for (command, expected) in [
        ("/code-model only", CodeMode::Only),
        ("/code-model off", CodeMode::Off),
        ("/code-model on", CodeMode::On),
    ] {
        let mode = AgentMode::Plan.command(command).unwrap();
        assert!(save_default_for_command(&paths, command, mode).unwrap());
        let saved = AppConfig::load(&paths).unwrap();
        assert_eq!(saved.codemode.mode, expected);
        assert_eq!(saved.codemode.inline_budget, 1234);
        assert_eq!(saved.codemode.timeout_ms, 4321);
        assert_eq!(serde_json::to_value(&saved.permission).unwrap(), permission);
        for surface in [PermissionSurface::Tui, PermissionSurface::Cli] {
            let resolved = resolve_agent_mode(&paths, None, surface).unwrap();
            assert_eq!(resolved.codemode(), expected);
        }
    }
}

/// 【工具方式】【临时覆盖】命令行参数覆盖本次启动，但不改写已经保存的工具方式。
/// 参数: 无；返回: 无。
#[test]
fn cli_override_does_not_replace_saved_default() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    save_default_for_command(
        &paths,
        "/code-model only",
        AgentMode::Audited.with_code_mode(CodeMode::Only),
    )
    .unwrap();
    let cli = Cli::try_parse_from(["sai", "--yolo", "--no-code-model"]).unwrap();
    let resolved =
        resolve_agent_mode(&paths, cli_mode_override(&cli), PermissionSurface::Tui).unwrap();
    assert_eq!(resolved, AgentMode::Yolo);
    assert_eq!(
        AppConfig::load(&paths).unwrap().codemode.mode,
        CodeMode::Only
    );
}

/// 【工具方式】【重复选择】即使当前模式不变，显式选择仍保存默认值；无配置时创建配置。
/// 参数: 无；返回: 无。
#[test]
fn explicit_selection_is_saved_even_without_a_session_mode_change() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    assert!(save_default_for_command(&paths, "/CODE-MODEL ON ", AgentMode::CodeModel).unwrap());
    assert!(paths.config_file.exists());
    assert_eq!(AppConfig::load(&paths).unwrap().codemode.mode, CodeMode::On);
    assert!(save_default_for_command(&paths, "/code-model off", AgentMode::Yolo).unwrap());
    assert_eq!(
        AppConfig::load(&paths).unwrap().codemode.mode,
        CodeMode::Off
    );
    let next = AgentMode::Yolo.command("/code-model").unwrap();
    assert!(save_default_for_command(&paths, "/code-model", next).unwrap());
    assert_eq!(AppConfig::load(&paths).unwrap().codemode.mode, CodeMode::On);
}

/// 【工具方式】【缺省固定】配置缺少工具方式时，显式启用会写入选择，不再依赖程序缺省值。
/// 参数: 无；返回: 无。
#[test]
fn explicit_default_is_materialized_when_the_setting_was_absent() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    paths.create_dirs().unwrap();
    let mut raw = serde_json::to_value(AppConfig::default()).unwrap();
    raw.as_object_mut().unwrap().remove("codemode");
    std::fs::write(&paths.config_file, serde_json::to_vec(&raw).unwrap()).unwrap();
    save_default_for_command(&paths, "/code-model on", AgentMode::CodeModel).unwrap();
    let saved: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&paths.config_file).unwrap()).unwrap();
    assert_eq!(saved["codemode"]["mode"], "on");
}

/// 【工具方式】【权限独立】权限命令和无效脚本命令不创建或改写全局配置。
/// 参数: 无；返回: 无。
#[test]
fn permission_commands_do_not_persist_tool_defaults() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    for command in ["/yolo", "/audit", "/auto", "/plan", "/code-model unknown"] {
        assert!(!save_default_for_command(&paths, command, AgentMode::CodeModel).unwrap());
    }
    assert!(!paths.config_file.exists());
}

/// 【工具方式】【保存边界】读取失败不会覆盖损坏配置，也不会报告保存成功。
/// 参数: 无；返回: 无。
#[test]
fn invalid_config_is_not_overwritten() {
    let root = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(root.path());
    paths.create_dirs().unwrap();
    std::fs::write(&paths.config_file, "invalid configuration").unwrap();
    assert!(save_default_for_command(&paths, "/code-model on", AgentMode::CodeModel).is_err());
    assert_eq!(
        std::fs::read_to_string(&paths.config_file).unwrap(),
        "invalid configuration"
    );
}
