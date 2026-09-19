use super::{WindowsPasteKey, WindowsPasteState};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::fs::OpenOptions;
use std::os::windows::{io::AsRawHandle, process::CommandExt};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use windows_sys::Win32::System::Console::*;

/// 【终端】【粘贴回归】在独立 Windows 子进程验证原生队列，避免影响其他终端测试
/// 参数: 无；返回无，原生队列丢键或响应超时则断言失败
#[test]
fn windows_console_paste_keeps_followup_responsive() {
    let report = std::env::temp_dir().join(format!("sai-paste-{}.txt", std::process::id()));
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "cli::repl_windows_paste::native_tests::console_child",
            "--ignored",
            "--nocapture",
        ])
        .env("SAI_PASTE_TEST_REPORT", &report)
        .stdin(Stdio::null())
        // 1. 【终端】【粘贴回归】子进程拥有自己的控制台，不使用测试调用者的输入队列
        .creation_flags(0x08000000)
        .output()
        .unwrap();
    let measured = std::fs::read_to_string(&report).unwrap_or_default();
    let _ = std::fs::remove_file(report);
    assert!(
        output.status.success(),
        "{measured}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(measured.contains("PASS"), "原生测试未完成: {measured}");
    eprintln!("{measured}");
}

/// 【终端】【粘贴回归】创建专用控制台并执行原生事件测试
/// 参数: 无；返回无，仅由上面的隔离进程入口运行
#[test]
#[ignore = "由独立子进程启动，避免共享控制台输入"]
fn console_child() {
    assert!(std::env::var_os("SAI_PASTE_TEST_REPORT").is_some());
    unsafe {
        FreeConsole();
        assert_ne!(AllocConsole(), 0);
    }
    crossterm::terminal::enable_raw_mode().unwrap();
    let console = OpenOptions::new()
        .read(true)
        .write(true)
        .open("CONIN$")
        .unwrap();
    let body = "abcd中\n".repeat(4_000);
    let (elapsed, reads) = replay_text(&console, &body, true);
    let report = format!(
        "chars={} next_key_ms={} event_reads={reads}\n",
        body.chars().count(),
        elapsed.as_millis()
    );
    let report_path = std::env::var_os("SAI_PASTE_TEST_REPORT").unwrap();
    std::fs::write(&report_path, &report).unwrap();
    assert!(elapsed < Duration::from_millis(500), "{report}");
    assert!(reads < 100, "粘贴仍逐字读取: {report}");

    // 2. 【终端】【粘贴回归】让高代理落在批次末尾，核验 UTF-16 边界及制表换行
    replay_text(&console, &format!("{}𠮷\t中\n文", "a".repeat(2048)), false);
    preserves_shortcut(&console);
    preserves_resize(&console);
    empty_queue_returns_immediately(&console);
    std::fs::write(report_path, format!("{report}PASS\n")).unwrap();
}

/// 【终端】【粘贴回归】写入粘贴事件及后续普通键、回车、快捷键
/// 参数: console 为独立控制台，body 为正文，releases 表示是否交错释放事件
/// 返回: 首个后续按键延迟及普通事件读取次数
fn replay_text(console: &std::fs::File, body: &str, releases: bool) -> (Duration, usize) {
    clear_console(console);
    let mut state = prepared_state(body);
    let mut records = text_records(body, releases);
    records.extend(text_records("!\n", false));
    records.push(key_record(b'c' as u16, 0x43, LEFT_CTRL_PRESSED, true));
    write_records(console, &records);
    let started = Instant::now();
    let mut reads = 0;
    loop {
        let event = next_event();
        reads += 1;
        let Event::Key(key) = event else { continue };
        if key.kind == KeyEventKind::Release {
            continue;
        }
        let candidate = match key.code {
            KeyCode::Char(ch) => WindowsPasteKey::Char(ch),
            KeyCode::Enter => WindowsPasteKey::Enter,
            KeyCode::Tab => WindowsPasteKey::Tab,
            _ => panic!("非预期按键: {key:?}"),
        };
        if state.consume_key(candidate) {
            state.drain_console_replay();
            continue;
        }
        assert_eq!(key.code, KeyCode::Char('!'));
        break;
    }
    let elapsed = started.elapsed();
    assert!(state.replay.is_empty());
    assert_eq!(
        next_event(),
        Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
    );
    assert_eq!(
        next_event(),
        Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL))
    );
    (elapsed, reads)
}

/// 【终端】【粘贴回归】回放中间的快捷键必须保留，其后的队列保持不动
/// 参数: console 为独立控制台；返回无
fn preserves_shortcut(console: &std::fs::File) {
    clear_console(console);
    let mut state = prepared_state("abcd");
    let mut records = text_records("ab", false);
    records.push(key_record(b'c' as u16, 0x43, LEFT_CTRL_PRESSED, true));
    records.extend(text_records("cd", false));
    write_records(console, &records);
    assert_eq!(state.drain_console_replay(), 2);
    assert_eq!(state.replay_offset, 2);
    assert_eq!(
        next_event(),
        Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL))
    );
    assert_eq!(
        next_event(),
        Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE))
    );
    assert_eq!(
        next_event(),
        Event::Key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE))
    );
}

/// 【终端】【粘贴回归】尺寸事件不得被批量丢弃或越过
/// 参数: console 为独立控制台；返回无
fn preserves_resize(console: &std::fs::File) {
    clear_console(console);
    let mut state = prepared_state("ab");
    let mut resize: INPUT_RECORD = unsafe { std::mem::zeroed() };
    resize.EventType = WINDOW_BUFFER_SIZE_EVENT as u16;
    resize.Event.WindowBufferSizeEvent.dwSize = COORD { X: 80, Y: 24 };
    let mut records = text_records("a", false);
    records.push(resize);
    records.extend(text_records("b", false));
    write_records(console, &records);
    assert_eq!(state.drain_console_replay(), 1);
    assert!(matches!(next_event(), Event::Resize(_, _)));
    assert_eq!(
        next_event(),
        Event::Key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::NONE))
    );
}

/// 【终端】【粘贴回归】已确认粘贴但下一批尚未到达时不得等待
/// 参数: console 为独立控制台；返回无
fn empty_queue_returns_immediately(console: &std::fs::File) {
    clear_console(console);
    let mut state = prepared_state("pending");
    let started = Instant::now();
    assert_eq!(state.drain_console_replay(), 0);
    assert!(started.elapsed() < Duration::from_millis(100));
    assert_eq!(state.replay, "pending");
}

/// 【终端】【粘贴回归】创建首行已识别的回放状态
/// 参数: body 为首行之后的正文；返回粘贴状态
fn prepared_state(body: &str) -> WindowsPasteState {
    let now = Instant::now();
    let mut state = WindowsPasteState::default();
    state.record_text("first", now);
    assert!(state
        .begin_multiline("first", &format!("first\n{body}"), now)
        .is_some());
    state
}

/// 【终端】【粘贴回归】构造文本按下及可选释放记录
/// 参数: text 为正文，releases 控制释放记录；返回 UTF-16 控制台事件
fn text_records(text: &str, releases: bool) -> Vec<INPUT_RECORD> {
    let mut records = Vec::new();
    for unit in text.encode_utf16() {
        let (unit, virtual_key) = match unit {
            10 => (13, 13),
            9 => (9, 9),
            other => (other, 0),
        };
        records.push(key_record(unit, virtual_key, 0, true));
        if releases {
            records.push(key_record(unit, virtual_key, 0, false));
        }
    }
    records
}

/// 【终端】【粘贴回归】构造单条键盘记录
/// 参数: unit 为 UTF-16 单元，virtual_key 为虚拟键，modifiers 为控制键标志，pressed 表示按下
/// 返回: 原生输入记录
fn key_record(unit: u16, virtual_key: u16, modifiers: u32, pressed: bool) -> INPUT_RECORD {
    let mut record: INPUT_RECORD = unsafe { std::mem::zeroed() };
    record.EventType = KEY_EVENT as u16;
    record.Event.KeyEvent = KEY_EVENT_RECORD {
        bKeyDown: i32::from(pressed),
        wRepeatCount: 1,
        wVirtualKeyCode: virtual_key,
        wVirtualScanCode: 0,
        uChar: KEY_EVENT_RECORD_0 { UnicodeChar: unit },
        dwControlKeyState: modifiers,
    };
    record
}

/// 【终端】【粘贴回归】写入独立控制台输入队列
/// 参数: console 为句柄持有者，records 为待写记录；返回无
fn write_records(console: &std::fs::File, records: &[INPUT_RECORD]) {
    let mut written = 0;
    assert_ne!(
        unsafe {
            WriteConsoleInputW(
                console.as_raw_handle().cast(),
                records.as_ptr(),
                records.len() as u32,
                &mut written,
            )
        },
        0
    );
    assert_eq!(written as usize, records.len());
}

/// 【终端】【粘贴回归】清理测试子进程专用队列，不接触用户终端
/// 参数: console 为独立控制台；返回无
fn clear_console(console: &std::fs::File) {
    assert_ne!(
        unsafe { FlushConsoleInputBuffer(console.as_raw_handle().cast()) },
        0
    );
}

/// 【终端】【粘贴回归】有界等待下一事件，缺失时立即报告用例失败
/// 参数: 无；返回终端事件
fn next_event() -> Event {
    assert!(event::poll(Duration::from_secs(2)).unwrap(), "缺少后续按键");
    event::read().unwrap()
}
