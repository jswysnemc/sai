use anyhow::Result;
use std::io::{Cursor, Write};
use std::time::{Duration, Instant};

/// 【通知投递】【提示音】播放固定轻提示音，音频设备不可用时以终端响铃回退。
/// @returns 播放结果；回退失败不影响通知或对话
pub(super) fn play() -> Result<()> {
    const CHIME: &[u8] = include_bytes!("../assets/reply-chime.wav");
    if play_wav(CHIME).is_err() {
        let _ = std::io::stderr().write_all(b"\x07");
        let _ = std::io::stderr().flush();
    }
    Ok(())
}

/// 【通知投递】【音频设备】播放内存 WAV，最多等待四百毫秒。
/// @param bytes 固定 WAV 资源
/// @returns 音频初始化与解码结果
fn play_wav(bytes: &[u8]) -> Result<()> {
    let (_stream, handle) = rodio::OutputStream::try_default()?;
    let sink = rodio::Sink::try_new(&handle)?;
    let source = rodio::Decoder::new(Cursor::new(bytes.to_vec()))?;
    sink.append(source);
    let deadline = Instant::now() + Duration::from_millis(400);
    while !sink.empty() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    sink.stop();
    Ok(())
}
