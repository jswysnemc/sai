use super::Cli;
use crate::config::TuiStyle;
use clap::Parser;

/// 【终端】【样式测试】启动选项在主命令和子命令中可用，非法值拒绝启动。
/// 参数: 无；返回: 无。
#[test]
fn tui_style_cli_option_is_global_and_validated() {
    assert_eq!(Cli::try_parse_from(["sai"]).unwrap().style, None);
    for arguments in [
        vec!["sai", "--style", "deepseek"],
        vec!["sai", "resume", "--style", "deepseek"],
        vec!["sai", "--style", "deepseek", "sessions"],
    ] {
        assert_eq!(
            Cli::try_parse_from(arguments).unwrap().style,
            Some(TuiStyle::Deepseek)
        );
    }
    assert_eq!(
        Cli::try_parse_from(["sai", "--style", "classic"])
            .unwrap()
            .style,
        Some(TuiStyle::Classic)
    );
    assert!(Cli::try_parse_from(["sai", "--style", "other"]).is_err());
}
