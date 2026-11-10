//! 从 crossterm 0.28 的事件流还原 Kitty APC 应答，其余事件保持原样。

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};

const MAX_RESPONSE: usize = 1024;
const PREFIX: &str = "\x1b_G";

pub(super) struct ProbeResponse {
    id: u32,
    text: String,
    pending: Vec<usize>,
    input: Vec<Option<Event>>,
}

impl ProbeResponse {
    /// 【终端图片】【应答匹配】只接受本次查询编号，避免无关输入被误判为协议支持。
    /// 参数: id 为查询使用的图片编号；返回: 空解析状态。
    pub(super) fn new(id: u32) -> Self {
        Self {
            id,
            text: String::new(),
            pending: Vec::new(),
            input: Vec::new(),
        }
    }

    /// 【终端图片】【应答解析】兼容 Alt 编码及独立 Esc 事件，不消耗普通键、粘贴或鼠标。
    /// 参数: event 为已有读取器解析的事件；返回: 完整匹配应答的成功状态，未完成为 None。
    pub(super) fn push(&mut self, event: Event) -> Option<bool> {
        let fragment = fragment(&event);
        let index = self.input.len();
        self.input.push(Some(event));
        let Some(fragment) = fragment else {
            // 1. 【终端图片】【事件保留】窗口尺寸、鼠标和粘贴可能穿插到应答事件之间
            return None;
        };
        // 2. 【终端图片】【序列边界】新的 APC 起点不能与用户先前单独按下的 Esc 混合
        if fragment.starts_with('\x1b')
            && !self.text.is_empty()
            && !(self.text.starts_with(PREFIX) && (fragment == "\x1b" || fragment == "\x1b\\"))
        {
            self.reset_pending();
        }
        self.text.push_str(&fragment);
        self.pending.push(index);
        if PREFIX.starts_with(&self.text) {
            return None;
        }
        if let Some(body) = self.text.strip_prefix(PREFIX) {
            if let Some(body) = body.strip_suffix("\x1b\\") {
                let supported = matching_status(body, self.id);
                self.discard_response();
                return supported;
            }
            if self.text.len() <= MAX_RESPONSE {
                return None;
            }
            self.discard_response();
            return Some(false);
        }
        self.reset_pending();
        None
    }

    /// 【终端图片】【输入归还】归还普通事件，完整查询前缀之后的残缺应答不会写入草稿。
    /// 参数: self 为查询结束时的状态；返回: 按接收顺序保存的普通事件。
    pub(super) fn into_input(mut self) -> Vec<Event> {
        if self.text.starts_with(PREFIX) {
            self.discard_response();
        }
        self.input.into_iter().flatten().collect()
    }

    /// 【终端图片】【候选撤回】前缀不匹配时把候选事件完整交还用户输入。
    /// 参数: 无；返回: 无。
    fn reset_pending(&mut self) {
        self.pending.clear();
        self.text.clear();
    }

    /// 【终端图片】【应答消费】移除图形协议响应事件，穿插输入保留原始位置。
    /// 参数: 无；返回: 无。
    fn discard_response(&mut self) {
        for index in self.pending.drain(..) {
            self.input[index] = None;
        }
        self.text.clear();
    }
}

/// 【终端图片】【编号校验】只有对应查询编号的 OK 应答可以启用图形协议。
/// 参数: body 为 APC 正文，id 为查询编号；返回: 匹配状态或 None。
fn matching_status(body: &str, id: u32) -> Option<bool> {
    let (control, status) = body.split_once(';')?;
    let response_id = control.split(',').find_map(|field| {
        field
            .strip_prefix("i=")
            .and_then(|value| value.parse::<u32>().ok())
    });
    (response_id == Some(id)).then_some(status == "OK")
}

/// 【终端图片】【转义还原】crossterm 将 APC 的 ESC 前导解析成 Alt 键或独立 Esc。
/// 参数: event 为终端事件；返回: 可参与 ASCII 协议匹配的片段，其他事件为 None。
fn fragment(event: &Event) -> Option<String> {
    let Event::Key(key) = event else {
        return None;
    };
    if key.kind != KeyEventKind::Press
        || key.modifiers.intersects(
            KeyModifiers::CONTROL | KeyModifiers::SUPER | KeyModifiers::HYPER | KeyModifiers::META,
        )
    {
        return None;
    }
    match key.code {
        KeyCode::Esc => Some("\x1b".to_string()),
        KeyCode::Char(ch) if ch.is_ascii() && !ch.is_control() => {
            let prefix = if key.modifiers.contains(KeyModifiers::ALT) {
                "\x1b"
            } else {
                ""
            };
            Some(format!("{prefix}{ch}"))
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "probe_response_tests.rs"]
mod tests;
