use crate::cli::repl_runtime::ReplRuntime;
use crate::cli::repl_windows_paste::WindowsPasteKey;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use std::io;
use std::time::Duration;

const MAX_TEXT_BATCH_CHARS: usize = 512;

/// 【终端】【输入批处理】读取已排队或立即可用的事件，避免跨轮次丢失按键。
/// 参数: `runtime` 为输入事件队列的持有者
/// 返回: 下一事件；当前没有输入时为空
pub(in crate::cli) fn next_ready_event(runtime: &mut ReplRuntime) -> io::Result<Option<Event>> {
    if let Some(event) = runtime.pop_input_event() {
        return Ok(Some(event));
    }
    if event::poll(Duration::ZERO)? {
        event::read().map(Some)
    } else {
        Ok(None)
    }
}

/// 【终端】【输入批处理】合并连续普通字符，将首个控制事件放回队列头。
/// 参数: `first` 为已读取字符，`runtime` 为终端运行期
/// 返回: 应当一次插入并绘制的文字
pub(in crate::cli) fn take_text_batch(
    first: char,
    runtime: &mut ReplRuntime,
) -> io::Result<String> {
    let (text, pending) = collect_text(first, || next_ready_event(runtime))?;
    if let Some(event) = pending {
        runtime.return_input_event(event);
    }
    Ok(text)
}

/// 【终端】【输入批处理】有界收集普通字符，不延迟普通键入且不跨过回车或快捷键。
/// 参数: `first` 为首字符，`next` 为非阻塞事件读取器
/// 返回: 合并文字与尚未处理的首个非文字事件
fn collect_text(
    first: char,
    mut next: impl FnMut() -> io::Result<Option<Event>>,
) -> io::Result<(String, Option<Event>)> {
    let mut text = String::from(first);
    for _ in 1..MAX_TEXT_BATCH_CHARS {
        let Some(event) = next()? else { break };
        if let Event::Key(key) = &event {
            if key.kind != KeyEventKind::Release {
                if let KeyCode::Char(ch) = key.code {
                    if !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                        && !ch.is_control()
                    {
                        text.push(ch);
                        continue;
                    }
                }
            }
        }
        return Ok((text, Some(event)));
    }
    Ok((text, None))
}

/// 【终端】【Windows 粘贴】把可回放按键映射为文字、换行或制表符。
/// 参数: `code` 为键码，`modifiers` 为修饰键
/// 返回: 可参与粘贴回放匹配的按键
pub(in crate::cli) fn windows_paste_key(
    code: KeyCode,
    modifiers: KeyModifiers,
) -> Option<WindowsPasteKey> {
    match code {
        KeyCode::Char(ch) if !modifiers.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) => {
            Some(WindowsPasteKey::Char(ch))
        }
        KeyCode::Enter if modifiers.is_empty() => Some(WindowsPasteKey::Enter),
        KeyCode::Tab if modifiers.is_empty() => Some(WindowsPasteKey::Tab),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyEvent;
    use std::collections::VecDeque;

    /// 【终端】【输入批处理】混合文字一次处理，回车仍按原顺序留给提交逻辑。
    /// 参数: 无
    /// 返回: 无，事件次序或内容错误时断言失败
    #[test]
    fn preserves_unicode_and_stops_before_submit() {
        let enter = Event::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let mut events: VecDeque<_> = "文abc"
            .chars()
            .map(|ch| Event::Key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)))
            .collect();
        events.push_back(enter.clone());
        events.push_back(Event::Resize(80, 24));
        let (text, pending) = collect_text('中', || Ok(events.pop_front())).unwrap();
        assert_eq!(text, "中文abc");
        assert_eq!(pending, Some(enter));
        assert_eq!(events.pop_front(), Some(Event::Resize(80, 24)));
    }

    /// 【终端】【输入批处理】长输入分批处理，为模型输出与终端刷新保留执行机会。
    /// 参数: 无
    /// 返回: 无，单批输入没有上限时断言失败
    #[test]
    fn bounds_one_batch_without_losing_the_next_character() {
        let mut count = 0;
        let (text, pending) = collect_text('x', || {
            count += 1;
            Ok(Some(Event::Key(KeyEvent::new(
                KeyCode::Char('x'),
                KeyModifiers::NONE,
            ))))
        })
        .unwrap();
        assert_eq!(text.len(), MAX_TEXT_BATCH_CHARS);
        assert_eq!(count, MAX_TEXT_BATCH_CHARS - 1);
        assert!(pending.is_none());
    }
}
