use super::Cli;
use crate::config::TuiMode;
use clap::Parser;

/// 【终端】【样式测试】启动选项在主命令和子命令中可用，非法值拒绝启动。
/// 参数: 无；返回: 无。
#[test]
fn tui_mode_cli_option_is_global_and_validated() {
    assert_eq!(Cli::try_parse_from(["sai"]).unwrap().view, None);
    for arguments in [
        vec!["sai", "--view", "fullscreen"],
        vec!["sai", "resume", "--view", "fullscreen"],
        vec!["sai", "--view", "fullscreen", "sessions"],
    ] {
        assert_eq!(
            Cli::try_parse_from(arguments).unwrap().view,
            Some(TuiMode::Fullscreen)
        );
    }
    assert_eq!(
        Cli::try_parse_from(["sai", "--view", "inline"])
            .unwrap()
            .view,
        Some(TuiMode::Inline)
    );
    assert!(Cli::try_parse_from(["sai", "--view", "other"]).is_err());
}
