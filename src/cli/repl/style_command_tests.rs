use super::*;

/// 【终端】【样式测试】验证切换参数、命令边界及运行期间命令隔离。
/// 参数: 无；返回: 无。
#[test]
fn tui_style_command_validates_arguments_and_stream_policy() {
    assert_eq!(
        requested_style("", TuiStyle::Classic).unwrap(),
        TuiStyle::Deepseek
    );
    assert_eq!(
        requested_style("", TuiStyle::Deepseek).unwrap(),
        TuiStyle::Classic
    );
    assert_eq!(
        requested_style("DEEPSEEK", TuiStyle::Classic).unwrap(),
        TuiStyle::Deepseek
    );
    for invalid in ["other", "classic extra", "deepseek classic"] {
        assert!(requested_style(invalid, TuiStyle::Classic).is_err());
    }
    assert_eq!(
        repl_command_rest("/STYLE deepseek", "/style"),
        Some("deepseek")
    );
    assert_eq!(repl_command_rest("/stylesheet", "/style"), None);
    assert!(crate::cli::repl_commands::repl_commands().contains(&"/style"));
    assert_eq!(
        crate::cli::repl_commands::complete_repl_command("/sty", false),
        Some("/style")
    );
    for input in ["/style", "/style deepseek", "/STYLE classic"] {
        assert_eq!(
            crate::cli::repl_commands::stream_command_policy(input),
            crate::cli::repl_commands::StreamCommandPolicy::Disabled,
        );
    }
}

/// 【终端】【样式测试】验证命令行覆盖优先级与配置刷新后的选择保留。
/// 参数: 无；返回: 无。
#[test]
fn tui_style_override_survives_config_refresh() {
    render::tui_style::set_override(None);
    render::tui_style::configure(TuiStyle::Classic);
    assert_eq!(render::tui_style::current(), TuiStyle::Classic);
    render::tui_style::set_override(Some(TuiStyle::Deepseek));
    render::tui_style::configure(TuiStyle::Classic);
    assert_eq!(render::tui_style::current(), TuiStyle::Deepseek);
    render::tui_style::set_override(None);
    render::tui_style::configure(TuiStyle::Deepseek);
    assert_eq!(render::tui_style::current(), TuiStyle::Deepseek);
    render::tui_style::configure(TuiStyle::Classic);
}

/// 【终端】【样式测试】保存展示偏好时保留供应商、全屏和其他配置。
/// 参数: 无；返回: 无。
#[test]
fn tui_style_save_preserves_unrelated_settings() {
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
    save_style(&paths, TuiStyle::Deepseek).unwrap();
    let stored = AppConfig::load(&paths).unwrap();
    assert_eq!(stored.display.tui_style, TuiStyle::Deepseek);
    let mut expected = serde_json::to_value(original).unwrap();
    expected["display"]["tui_style"] = "deepseek".into();
    assert_eq!(serde_json::to_value(stored).unwrap(), expected);
}
