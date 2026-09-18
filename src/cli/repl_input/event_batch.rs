use crate::cli::repl_runtime::ReplRuntime;
use crate::cli::repl_windows_paste::WindowsPasteKey;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use std::io;
use std::time::Duration;

const MAX_TEXT_BATCH_CHARS: usize = 512;
const MAX_TEXT_BATCH_EVENTS: usize = MAX_TEXT_BATCH_CHARS * 2;

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
    let mut chars = 1;
    for _ in 1..MAX_TEXT_BATCH_EVENTS {
        if chars == MAX_TEXT_BATCH_CHARS {
            break;
        }
        let Some(event) = next()? else { break };
        if let Event::Key(key) = &event {
            // 1. 【终端】【Windows 粘贴】释放事件不修改输入，跳过它们以合并后续按下事件
            if key.kind == KeyEventKind::Release {
                continue;
            }
            if let KeyCode::Char(ch) = key.code {
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                    && !ch.is_control()
                {
                    text.push(ch);
                    chars += 1;
                    continue;
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

    /// 【终端】【输入调度】连续释放事件也有读取上限，不独占输入处理循环。
    /// 参数: 无
    /// 返回: 无；释放事件导致无限读取时断言失败
    #[test]
    fn release_only_stream_has_a_bounded_read_budget() {
        let mut reads = 0;
        let (text, pending) = collect_text('a', || {
            reads += 1;
            assert!(reads < MAX_TEXT_BATCH_EVENTS);
            Ok(Some(Event::Key(KeyEvent::new_with_kind(
                KeyCode::Char('a'),
                KeyModifiers::NONE,
                KeyEventKind::Release,
            ))))
        })
        .unwrap();
        assert_eq!(text, "a");
        assert_eq!(reads, MAX_TEXT_BATCH_EVENTS - 1);
        assert!(pending.is_none());
    }

    /// 【终端】【Windows 粘贴】交错的按下与释放事件不得把大段输入拆成逐字绘制。
    /// 参数: 无
    /// 返回: 无；一万字符需要超过有界字符批次数时断言失败
    #[test]
    fn windows_paste_release_events_do_not_force_per_character_redraws() {
        let text = "中文abc".repeat(2_000);
        let mut events = VecDeque::new();
        for ch in text.chars() {
            for kind in [KeyEventKind::Press, KeyEventKind::Release] {
                events.push_back(Event::Key(KeyEvent::new_with_kind(
                    KeyCode::Char(ch),
                    KeyModifiers::NONE,
                    kind,
                )));
            }
        }
        let interrupt = Event::Key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        events.push_back(interrupt.clone());
        let mut actual = String::new();
        let mut redraws = 0;
        let mut interrupted = false;
        let started = std::time::Instant::now();
        while let Some(event) = events.pop_front() {
            if event == interrupt {
                interrupted = true;
                break;
            }
            let Event::Key(key) = event else {
                panic!("unexpected event")
            };
            if key.kind == KeyEventKind::Release {
                continue;
            }
            let KeyCode::Char(first) = key.code else {
                panic!("unexpected key")
            };
            let (batch, pending) = collect_text(first, || Ok(events.pop_front())).unwrap();
            actual.push_str(&batch);
            redraws += 1;
            std::hint::black_box(crate::cli::repl_input_render::repl_visible_input_lines(
                "",
                &[actual.clone()],
                crate::cli::REPL_MAX_VISIBLE_INPUT_ROWS,
                false,
            ));
            if let Some(pending) = pending {
                events.push_front(pending);
            }
        }
        assert_eq!(actual, text);
        assert!(interrupted, "粘贴之后的 Ctrl+C 必须留给中断处理");
        assert!(events.is_empty());
        eprintln!(
            "Windows paste: chars={}, redraws={redraws}, elapsed={:?}",
            text.chars().count(),
            started.elapsed()
        );
        assert_eq!(redraws, text.chars().count().div_ceil(MAX_TEXT_BATCH_CHARS));
    }
}
