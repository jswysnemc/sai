use crate::config::{AppConfig, NotificationConfig};
use anyhow::Result;
use std::io::{Cursor, Write};
use std::process::Command;
use std::thread;
use std::time::Duration;

/// 在 TUI 完成答复后发送桌面通知与可选提示音。
///
/// CLI 单次命令不调用此函数。
///
/// 参数:
/// - `config`: 应用配置
/// - `title`: 通知标题
/// - `body`: 通知正文摘要
///
/// 返回:
/// - 无（失败静默，不影响主流程）
pub fn notify_reply_complete(config: &AppConfig, title: &str, body: &str) {
    notify_reply_complete_with(&config.notification, title, body);
}

/// 使用显式通知配置发送完成提醒。
///
/// 参数:
/// - `settings`: 通知开关
/// - `title`: 通知标题
/// - `body`: 通知正文
///
/// 返回:
/// - 无
pub fn notify_reply_complete_with(settings: &NotificationConfig, title: &str, body: &str) {
    if !settings.enabled && !settings.sound {
        return;
    }
    let title = title.to_string();
    let body = truncate(body, 240);
    let enabled = settings.enabled;
    let sound = settings.sound;
    // 1. 后台线程发送，避免阻塞 TUI 事件循环
    let _ = thread::Builder::new()
        .name("sai-reply-notify".into())
        .spawn(move || {
            if enabled {
                let _ = send_desktop_notification(&title, &body);
            }
            if sound {
                let _ = play_reply_sound();
            }
        });
}

/// 截断正文为适合通知气泡的长度。
fn truncate(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim().replace('\n', " ");
    if trimmed.chars().count() <= max_chars {
        return trimmed;
    }
    let mut out: String = trimmed.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// 调用系统桌面通知。
fn send_desktop_notification(title: &str, body: &str) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        if Command::new("notify-send")
            .args(["--app-name=Sai", "--urgency=normal", title, body])
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
        {
            return Ok(());
        }
    }
    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "display notification \"{}\" with title \"{}\"",
            body.replace('\\', "\\\\").replace('"', "\\\""),
            title.replace('\\', "\\\\").replace('"', "\\\"")
        );
        if Command::new("osascript")
            .args(["-e", &script])
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
        {
            return Ok(());
        }
    }
    #[cfg(windows)]
    {
        let _ = (title, body);
    }
    Ok(())
}

/// 播放简短完成提示音；失败时终端响铃一次。
///
/// 使用独立轻提示音，不复用闹钟资源。
fn play_reply_sound() -> Result<()> {
    const REPLY_WAV: &[u8] = include_bytes!("assets/reply-chime.wav");
    match play_wav_bytes(REPLY_WAV) {
        Ok(()) => Ok(()),
        Err(_) => {
            let _ = std::io::stderr().write_all(b"\x07");
            let _ = std::io::stderr().flush();
            Ok(())
        }
    }
}

/// 使用 rodio 播放内存中的 wav 数据。
fn play_wav_bytes(bytes: &[u8]) -> Result<()> {
    let (_stream, handle) = rodio::OutputStream::try_default()?;
    let sink = rodio::Sink::try_new(&handle)?;
    let source = rodio::Decoder::new(Cursor::new(bytes.to_vec()))?;
    sink.append(source);
    // 轻提示音很短，最多等约 400ms
    let deadline = std::time::Instant::now() + Duration::from_millis(400);
    while !sink.empty() && std::time::Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
    }
    sink.stop();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_keeps_short_text() {
        assert_eq!(truncate("hello", 10), "hello");
        assert!(truncate("abcdefghijklmnopqrstuvwxyz", 10).ends_with('…'));
    }

    #[test]
    fn disabled_settings_do_nothing() {
        notify_reply_complete_with(
            &NotificationConfig {
                enabled: false,
                sound: false,
            },
            "t",
            "b",
        );
    }
}
