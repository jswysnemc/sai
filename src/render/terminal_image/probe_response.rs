//! 共用 Kitty 与设备属性应答解析器，保留 Unix 事件或 Windows 原生记录。

use crossterm::event::Event;
#[cfg(any(not(windows), test))]
use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};

const MAX_RESPONSE: usize = 1024;
const PREFIX: &str = "\x1b_G";
const DEVICE_PREFIX: &str = "\x1b[?";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ProbeReply {
    Kitty(bool),
    Sixel(bool),
}

pub(super) struct ProbeResponse<T = Event> {
    id: u32,
    text: String,
    pending: Vec<usize>,
    input: Vec<Option<T>>,
}

impl<T> ProbeResponse<T> {
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
    /// 参数: fragment 为可匹配协议的文本片段，event 为原始事件或记录；返回: 完整应答，未完成为 None。
    pub(super) fn push_fragment(
        &mut self,
        fragment: Option<String>,
        event: T,
    ) -> Option<ProbeReply> {
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
        if PREFIX.starts_with(&self.text) || DEVICE_PREFIX.starts_with(&self.text) {
            return None;
        }
        if let Some(body) = self.text.strip_prefix(PREFIX) {
            if let Some(body) = body.strip_suffix("\x1b\\") {
                let supported = matching_status(body, self.id);
                self.discard_response();
                return supported.map(ProbeReply::Kitty);
            }
            if self.text.len() <= MAX_RESPONSE {
                return None;
            }
            self.discard_response();
            return Some(ProbeReply::Kitty(false));
        }
        if let Some(body) = self.text.strip_prefix(DEVICE_PREFIX) {
            if let Some(attributes) = body.strip_suffix('c') {
                let supported = sixel_attribute(attributes);
                self.discard_response();
                return supported.map(ProbeReply::Sixel);
            }
            if self.text.len() <= MAX_RESPONSE
                && body.bytes().all(|ch| ch.is_ascii_digit() || ch == b';')
            {
                return None;
            }
        }
        self.reset_pending();
        None
    }

    /// 【终端图片】【输入归还】归还普通事件，完整查询前缀之后的残缺应答不会写入草稿。
    /// 参数: self 为查询结束时的状态；返回: 按接收顺序保存的普通事件。
    pub(super) fn into_input(mut self) -> Vec<T> {
        self.finish_query();
        self.input.into_iter().flatten().collect()
    }

    /// 【终端图片】【查询边界】切换查询前清理残缺响应，保留尚未确认属于协议的输入。
    /// 参数: 无；返回: 无。
    pub(super) fn finish_query(&mut self) {
        if self.text.starts_with(PREFIX) || self.text.starts_with(DEVICE_PREFIX) {
            self.discard_response();
        }
        self.reset_pending();
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

#[cfg(any(not(windows), test))]
impl ProbeResponse<Event> {
    /// 【终端图片】【事件适配】将 Unix 事件流送入共用解析器，保留原有事件对象。
    /// 参数: event 为终端事件；返回: 已识别的协议应答或 None。
    pub(super) fn push(&mut self, event: Event) -> Option<ProbeReply> {
        self.push_fragment(fragment(&event), event)
    }
}

/// 【终端图片】【Sixel 能力】检查主设备属性中的能力 4，不把设备型号当成能力。
/// 参数: attributes 为 DA1 数值字段；返回: 合法应答中的 Sixel 支持状态，畸形应答为 None。
fn sixel_attribute(attributes: &str) -> Option<bool> {
    let fields: Option<Vec<u16>> = attributes
        .split(';')
        .map(|value| value.parse().ok())
        .collect();
    let fields = fields?;
    Some(fields.iter().skip(1).any(|value| *value == 4))
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
#[cfg(any(not(windows), test))]
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
