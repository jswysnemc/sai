use super::*;

/// 【终端】【视图测试】验证切换参数、命令边界及运行期间命令隔离。
/// 参数: 无；返回: 无。
#[test]
fn tui_mode_command_validates_arguments_and_stream_policy() {
    assert_eq!(
        requested_view("", TuiMode::Inline).unwrap(),
        TuiMode::Fullscreen
    );
    assert_eq!(
        requested_view("", TuiMode::Fullscreen).unwrap(),
        TuiMode::Inline
    );
    assert_eq!(
        requested_view("FULLSCREEN", TuiMode::Inline).unwrap(),
        TuiMode::Fullscreen
    );
    for invalid in ["other", "inline extra", "fullscreen inline"] {
        assert!(requested_view(invalid, TuiMode::Inline).is_err());
    }
    assert_eq!(
        repl_command_rest("/VIEW fullscreen", "/view"),
        Some("fullscreen")
    );
    assert_eq!(repl_command_rest("/viewsheet", "/view"), None);
    assert!(crate::cli::repl_commands::repl_commands().contains(&"/view"));
    assert_eq!(
        crate::cli::repl_commands::complete_repl_command("/vie", false),
        Some("/view")
    );
    for input in ["/view", "/view fullscreen", "/VIEW inline"] {
        assert_eq!(
            crate::cli::repl_commands::stream_command_policy(input),
            crate::cli::repl_commands::StreamCommandPolicy::Disabled,
        );
    }
}

/// 【终端】【视图测试】验证命令行覆盖优先级与配置刷新后的选择保留。
/// 参数: 无；返回: 无。
#[test]
fn tui_mode_override_survives_config_refresh() {
    render::tui_mode::set_override(None);
    render::tui_mode::configure(TuiMode::Inline);
    assert_eq!(render::tui_mode::current(), TuiMode::Inline);
    render::tui_mode::set_override(Some(TuiMode::Fullscreen));
    render::tui_mode::configure(TuiMode::Inline);
    assert_eq!(render::tui_mode::current(), TuiMode::Fullscreen);
    render::tui_mode::set_override(None);
    render::tui_mode::configure(TuiMode::Fullscreen);
    assert_eq!(render::tui_mode::current(), TuiMode::Fullscreen);
    render::tui_mode::configure(TuiMode::Inline);
}

/// 【终端】【视图测试】保存展示偏好时保留供应商、全屏和其他配置。
/// 参数: 无；返回: 无。
#[test]
fn tui_mode_save_preserves_unrelated_settings() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let paths = SaiPaths {
        config_dir: root.join("config"),
        config_file: root.join("config/config.jsonc"),
        secrets_file: root.join("config/secrets.jsonc"),
        skills_dir: root.join("config/skills"),
        data_dir: root.join("data"),
        cache_dir: root.join("cache"),
        state_dir: root.join("state"),
        pictures_dir: root.join("pictures"),
        fish_hook_file: root.join("fish/sai.fish"),
        bash_hook_file: root.join("shell/bash-hook.sh"),
        zsh_hook_file: root.join("shell/zsh-hook.zsh"),
        powershell_hook_file: root.join("shell/powershell-hook.ps1"),
    };
    let config = AppConfig::default();
    config.save(&paths).unwrap();
    let original = AppConfig::load(&paths).unwrap();
    save_view(&paths, TuiMode::Fullscreen).unwrap();
    let stored = AppConfig::load(&paths).unwrap();
    assert!(stored.display.fullscreen);
    let mut expected = serde_json::to_value(original).unwrap();
    expected["display"]["fullscreen"] = true.into();
    assert_eq!(serde_json::to_value(stored).unwrap(), expected);
}
