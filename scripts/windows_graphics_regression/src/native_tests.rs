use super::console_fixture::{self, ConsoleFixture};
use super::terminal_fixture::TerminalFixture;
use std::process::Command;
use std::time::Duration;

const CASES: &[&str] = &[
    "startup",
    "success",
    "fragmented",
    "translated",
    "error",
    "wrong-id",
    "timeout",
    "sixel",
    "sixel-absent",
    "partial",
    "write-error",
    "redirected",
];

/// 【终端测试】【进程隔离】每个场景使用独立控制台和缓存，测试不修改父进程的终端。
/// 参数: 无；返回: 无，失败时附上子进程与断言报告。
#[test]
fn windows_graphics_regression() {
    for case in CASES {
        let report =
            std::env::temp_dir().join(format!("sai-graphics-{}-{case}.txt", std::process::id()));
        let result = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "native_tests::native_console_case",
                "--ignored",
                "--nocapture",
            ])
            .env("SAI_WINDOWS_GRAPHICS_CASE", case)
            .env("SAI_WINDOWS_GRAPHICS_REPORT", &report)
            .output()
            .unwrap();
        let diagnostic = std::fs::read_to_string(&report).unwrap_or_default();
        assert!(
            result.status.success(),
            "{case}: {diagnostic}\n{}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(diagnostic, "passed", "{case}: missing completion report");
        let _ = std::fs::remove_file(report);
        println!("{case}: passed");
    }
}

/// 【终端测试】【原生场景】直接调用生产查询，检查能力、输入记录及控制台模式。
/// 参数: 场景来自测试环境变量；返回: 无，断言详情写入隔离报告。
#[test]
#[ignore = "由父测试在独立进程执行"]
fn native_console_case() {
    let case = std::env::var("SAI_WINDOWS_GRAPHICS_CASE").unwrap();
    let report = std::path::PathBuf::from(std::env::var_os("SAI_WINDOWS_GRAPHICS_REPORT").unwrap());
    let failure_report = report.clone();
    std::panic::set_hook(Box::new(move |info| {
        let _ = std::fs::write(&failure_report, info.to_string());
    }));
    let console = ConsoleFixture::new();
    if case == "startup" {
        // 1. 【终端测试】【Ely 环境】实际平台入口不能受通用 TERM 或继承的 WT_SESSION 影响
        std::env::set_var("TERM", "xterm-256color");
        std::env::set_var("TERM_PROGRAM", "elyterminal");
        std::env::set_var("WT_SESSION", "inherited-session");
        std::env::remove_var("TMUX");
        let input = console.input() as usize;
        let responder = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            console_fixture::write_text(
                input as _,
                &format!("\x1b_Gi={};OK\x1b\\", crate::probe::QUERY_ID),
            );
        });
        assert!(crate::probe::initialize_terminal_graphics().is_empty());
        responder.join().unwrap();
        assert_eq!(crate::probe::kitty_supported(), Some(true));
    } else if case == "redirected" {
        let path = report.with_extension("redirected");
        let file = std::fs::File::create(&path).unwrap();
        console.redirect_output(&file);
        assert!(crate::probe::initialize_terminal_graphics().is_empty());
        assert_eq!(crate::probe::kitty_supported(), None);
        assert_eq!(file.metadata().unwrap().len(), 0);
        console.restore_output();
        drop(file);
        let _ = std::fs::remove_file(path);
    } else {
        let mut terminal = TerminalFixture::new(&case, &console);
        let result = crate::probe_windows::query_with_output(&mut terminal);
        terminal.finish();
        if case == "write-error" {
            assert!(result.is_err());
        } else {
            let capabilities = result.unwrap();
            assert_eq!(
                capabilities.kitty,
                matches!(case.as_str(), "success" | "fragmented" | "translated")
            );
            assert_eq!(capabilities.sixel, Some(case == "sixel"));
        }
        terminal.assert_input_preserved(&console);
        assert_eq!(
            terminal.requests,
            if matches!(
                case.as_str(),
                "success" | "fragmented" | "translated" | "write-error"
            ) {
                1
            } else {
                2
            }
        );
    }
    console.assert_modes_restored();
    drop(console);
    std::fs::write(report, "passed").unwrap();
}
