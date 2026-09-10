use super::delivery::DeliveryControl;
use anyhow::{Context, Result};
use std::io::Cursor;
use std::time::Duration;

/// 【通知投递】【音频播放】播放内存 WAV 或 MP3，取消与超时都会停止音频并释放设备。
/// @param bytes 有界内存音频；control 为可信取消信号与截止时间
/// @returns 完整播放成功或明确错误，不隐式选择其他投递通道
pub(super) fn play(bytes: &[u8], control: &DeliveryControl) -> Result<()> {
    // 1. 【通知投递】【音频准备】先解析音频，再打开设备；每个原生调用后重新检查取消
    control.check()?;
    let source =
        rodio::Decoder::new(Cursor::new(bytes.to_vec())).context("decode notification audio")?;
    control.check()?;
    let (_stream, handle) =
        rodio::OutputStream::try_default().context("open notification audio device")?;
    control.check()?;
    let sink = rodio::Sink::try_new(&handle).context("create notification audio sink")?;
    control.check()?;
    sink.append(source);
    // 2. 【通知投递】【播放等待】小间隔检查时限与取消，离开函数前停止并回收声音
    let result = (|| loop {
        control.check()?;
        if sink.empty() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(20));
    })();
    sink.stop();
    result
}
