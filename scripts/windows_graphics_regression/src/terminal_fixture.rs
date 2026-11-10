use super::console_fixture::{
    key_snapshot, text_records, write_records, write_text, ConsoleFixture,
};
use std::io::{self, Write};
use std::thread::JoinHandle;
use std::time::Duration;
use windows_sys::Win32::System::Console::*;

pub(super) struct TerminalFixture<'a> {
    case: &'a str,
    console: &'a ConsoleFixture,
    expected: Vec<INPUT_RECORD>,
    responder: Option<JoinHandle<()>>,
    pub(super) requests: usize,
}

impl<'a> TerminalFixture<'a> {
    /// 【终端测试】【终端夹具】配置查询应答，并准备真实按键、中文、代理对和释放事件。
    /// 参数: case 为场景，console 为原生控制台；返回: 模拟终端输出接收器。
    pub(super) fn new(case: &'a str, console: &'a ConsoleFixture) -> Self {
        Self {
            case,
            console,
            expected: Vec::new(),
            responder: None,
            requests: 0,
        }
    }

    /// 【终端测试】【响应完成】等待分段应答线程结束。
    /// 参数: 无；返回: 无。
    pub(super) fn finish(&mut self) {
        if let Some(responder) = self.responder.take() {
            responder.join().unwrap();
        }
    }

    /// 【终端测试】【输入完整性】协议不能残留在键盘队列，原始用户记录必须保持顺序与字段。
    /// 参数: console 为原生控制台；返回: 无。
    pub(super) fn assert_input_preserved(&self, console: &ConsoleFixture) {
        assert_eq!(
            key_snapshot(&console.remaining()),
            key_snapshot(&self.expected)
        );
    }
}

impl Write for TerminalFixture<'_> {
    /// 【终端测试】【请求应答】按程序实际发送的请求返回 Kitty 或设备属性响应。
    /// 参数: bytes 为请求；返回: 写入长度或用于恢复验证的模拟错误。
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.console.assert_query_modes();
        self.requests += 1;
        let kitty_query = bytes.starts_with(b"\x1b_G");
        assert!(kitty_query || bytes == b"\x1b[c", "unexpected query");
        let reply = if kitty_query {
            assert!(bytes.windows(3).any(|part| part == b"a=q"));
            match self.case {
                "success" | "fragmented" | "translated" => {
                    format!("\x1b_Gi={};OK\x1b\\", crate::probe::QUERY_ID)
                }
                "error" | "sixel" | "sixel-absent" => format!(
                    "\x1b_Gi={};EINVAL: unsupported\x1b\\",
                    crate::probe::QUERY_ID
                ),
                "wrong-id" => format!("\x1b_Gi={};OK\x1b\\", crate::probe::QUERY_ID + 1),
                "partial" => format!("\x1b_Gi={};O", crate::probe::QUERY_ID),
                _ => String::new(),
            }
        } else if self.case == "timeout" {
            String::new()
        } else if self.case == "sixel" {
            "\x1b[?61;4;6;22c".into()
        } else {
            "\x1b[?1;0c".into()
        };
        let mut before = text_records("中文\u{20000}z");
        before.last_mut().unwrap().Event.KeyEvent.wRepeatCount = 3;
        let mut release = text_records("x")[0];
        release.Event.KeyEvent.bKeyDown = 0;
        release.Event.KeyEvent.wVirtualKeyCode = 0x58;
        release.Event.KeyEvent.dwControlKeyState = LEFT_CTRL_PRESSED;
        before.push(release);
        // 1. 【终端测试】【残缺应答】不把用户文字插入未结束的协议帧，下一次查询继续验证输入
        let after = if self.case == "partial" && kitty_query {
            Vec::new()
        } else {
            text_records("end")
        };
        self.expected.extend_from_slice(&before);
        self.expected.extend_from_slice(&after);
        write_records(self.console.input(), &before);
        if self.case == "fragmented" {
            let input = self.console.input() as usize;
            self.responder = Some(std::thread::spawn(move || {
                for ch in reply.chars() {
                    write_text(input as _, &ch.to_string());
                    std::thread::sleep(Duration::from_millis(2));
                }
                write_text(input as _, "end");
            }));
        } else {
            if self.case == "translated" {
                let mut chars = reply.chars();
                while let Some(ch) = chars.next() {
                    if ch == '\x1b' {
                        let mut record = text_records(&chars.next().unwrap().to_string())[0];
                        record.Event.KeyEvent.dwControlKeyState = LEFT_ALT_PRESSED;
                        write_records(self.console.input(), &[record]);
                    } else {
                        write_text(self.console.input(), &ch.to_string());
                    }
                }
            } else if !reply.is_empty() {
                write_text(self.console.input(), &reply);
            }
            write_records(self.console.input(), &after);
        }
        if self.case == "write-error" {
            return Err(io::Error::other("fixture output failure"));
        }
        Ok(bytes.len())
    }

    /// 【终端测试】【输出提交】夹具同步注入应答，无额外缓冲。
    /// 参数: 无；返回: 成功。
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
