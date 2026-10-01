//! 按键解析：把 `Enter`、`Control+A` 这类描述转换为 CDP 键盘事件。

use super::session::BrowserSession;
use anyhow::{bail, Result};
use serde::Deserialize;
use serde_json::{json, Value};

/// CDP 修饰键位：Alt。
const MOD_ALT: i64 = 1;
/// CDP 修饰键位：Ctrl。
const MOD_CTRL: i64 = 2;
/// CDP 修饰键位：Meta（Command / Win）。
const MOD_META: i64 = 4;
/// CDP 修饰键位：Shift。
const MOD_SHIFT: i64 = 8;

/// 一个物理按键的协议描述。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct KeyDefinition {
    pub(super) key: String,
    pub(super) code: String,
    pub(super) key_code: i64,
    /// 按下时产生的字符；功能键为空
    pub(super) text: Option<String>,
}

/// 解析后的组合键。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct KeyChord {
    pub(super) modifiers: i64,
    pub(super) key: KeyDefinition,
}

/// 面板转发的一次原始键盘事件。
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct KeyInput {
    /// `down` 或 `up`
    pub(crate) kind: String,
    pub(crate) key: String,
    #[serde(default)]
    pub(crate) code: String,
    #[serde(default)]
    pub(crate) key_code: i64,
    /// 按下产生的字符，仅可打印键有值
    #[serde(default)]
    pub(crate) text: Option<String>,
    #[serde(default)]
    pub(crate) modifiers: i64,
}

/// 【内置浏览器】【功能键表】按名称查找功能键定义。
/// @param name 为按键名，大小写不敏感
/// @returns 功能键定义；不是功能键时为空
fn named_key(name: &str) -> Option<KeyDefinition> {
    let (key, code, key_code, text): (&str, &str, i64, Option<&str>) =
        match name.to_ascii_lowercase().as_str() {
            "enter" | "return" => ("Enter", "Enter", 13, Some("\r")),
            "tab" => ("Tab", "Tab", 9, None),
            "backspace" => ("Backspace", "Backspace", 8, None),
            "delete" | "del" => ("Delete", "Delete", 46, None),
            "escape" | "esc" => ("Escape", "Escape", 27, None),
            "space" => (" ", "Space", 32, Some(" ")),
            "arrowup" | "up" => ("ArrowUp", "ArrowUp", 38, None),
            "arrowdown" | "down" => ("ArrowDown", "ArrowDown", 40, None),
            "arrowleft" | "left" => ("ArrowLeft", "ArrowLeft", 37, None),
            "arrowright" | "right" => ("ArrowRight", "ArrowRight", 39, None),
            "home" => ("Home", "Home", 36, None),
            "end" => ("End", "End", 35, None),
            "pageup" => ("PageUp", "PageUp", 33, None),
            "pagedown" => ("PageDown", "PageDown", 34, None),
            "insert" => ("Insert", "Insert", 45, None),
            other => {
                let number = other.strip_prefix('f')?.parse::<i64>().ok()?;
                if !(1..=12).contains(&number) {
                    return None;
                }
                let name = format!("F{number}");
                return Some(KeyDefinition {
                    key: name.clone(),
                    code: name,
                    key_code: 111 + number,
                    text: None,
                });
            }
        };
    Some(KeyDefinition {
        key: key.to_string(),
        code: code.to_string(),
        key_code,
        text: text.map(str::to_string),
    })
}

/// 【内置浏览器】【字符键】把单个字符转换为按键定义。
/// @param ch 为字符；shift 为是否按住 Shift
/// @returns 按键定义
fn char_key(ch: char, shift: bool) -> KeyDefinition {
    let (code, key_code) = if ch.is_ascii_alphabetic() {
        (
            format!("Key{}", ch.to_ascii_uppercase()),
            ch.to_ascii_uppercase() as i64,
        )
    } else if ch.is_ascii_digit() {
        (format!("Digit{ch}"), ch as i64)
    } else {
        (String::new(), 0)
    };
    let text = if shift {
        ch.to_uppercase().to_string()
    } else {
        ch.to_string()
    };
    KeyDefinition {
        key: text.clone(),
        code,
        key_code,
        text: Some(text),
    }
}

/// 【内置浏览器】【组合键解析】解析 `Control+Shift+K` 形式的组合键。
/// @param input 为按键描述，修饰键与主键以 `+` 连接
/// @returns 组合键
pub(super) fn parse_chord(input: &str) -> Result<KeyChord> {
    let input = input.trim();
    if input.is_empty() {
        bail!("key is required");
    }
    // 单独的 "+" 本身就是主键
    let parts: Vec<&str> = if input == "+" {
        vec!["+"]
    } else {
        input.split('+').map(str::trim).collect()
    };
    let (main, modifier_names) = parts.split_last().expect("split yields at least one part");
    let mut modifiers = 0;
    for name in modifier_names {
        modifiers |= match name.to_ascii_lowercase().as_str() {
            "alt" | "option" => MOD_ALT,
            "control" | "ctrl" => MOD_CTRL,
            "meta" | "cmd" | "command" | "super" => MOD_META,
            "shift" => MOD_SHIFT,
            other => bail!("unknown modifier key: {other}"),
        };
    }
    let mut chars = main.chars();
    let key = match (chars.next(), chars.next()) {
        (Some(ch), None) => char_key(ch, modifiers & MOD_SHIFT != 0),
        _ => named_key(main).ok_or_else(|| anyhow::anyhow!("unknown key: {main}"))?,
    };
    Ok(KeyChord { modifiers, key })
}

/// 【内置浏览器】【编辑命令】为常见快捷键补充编辑命令，保证无头模式下全选、复制等生效。
/// @param modifiers 为修饰键位；key 为主键名
/// @returns 编辑命令列表
pub(super) fn editing_commands(modifiers: i64, key: &str) -> Vec<&'static str> {
    if modifiers & (MOD_CTRL | MOD_META) == 0 || modifiers & MOD_ALT != 0 {
        return Vec::new();
    }
    match key.to_ascii_lowercase().as_str() {
        "a" => vec!["selectAll"],
        "c" => vec!["copy"],
        "x" => vec!["cut"],
        "v" => vec!["paste"],
        "z" if modifiers & MOD_SHIFT != 0 => vec!["redo"],
        "z" => vec!["undo"],
        _ => Vec::new(),
    }
}

/// 【内置浏览器】【按键参数】生成一条 Input.dispatchKeyEvent 参数。
/// @param kind 为事件类型；modifiers 为修饰键；key 为按键定义
/// @returns 协议参数
fn key_event(kind: &str, modifiers: i64, key: &KeyDefinition) -> Value {
    let mut params = json!({
        "type": kind,
        "modifiers": modifiers,
        "key": key.key,
        "code": key.code,
        "windowsVirtualKeyCode": key.key_code,
        "nativeVirtualKeyCode": key.key_code,
    });
    // 按住 Ctrl/Meta 时不能附带字符，否则会把字母输入到页面
    let typing = modifiers & (MOD_CTRL | MOD_META) == 0;
    if kind == "keyDown" && typing {
        if let Some(text) = &key.text {
            params["text"] = json!(text);
            params["unmodifiedText"] = json!(text);
        }
    }
    if kind != "keyUp" {
        let commands = editing_commands(modifiers, &key.key);
        if !commands.is_empty() {
            params["commands"] = json!(commands);
        }
    }
    params
}

impl BrowserSession {
    /// 【内置浏览器】【组合键按下】依次按下修饰键、主键，再逆序抬起。
    /// @param input 为按键描述
    /// @returns 操作结果
    pub(crate) async fn press_key(&self, input: &str) -> Result<()> {
        let chord = parse_chord(input)?;
        // 1. 先按下修饰键，modifiers 逐个累加
        let modifier_keys = modifier_definitions(chord.modifiers);
        let mut held = 0;
        for (bit, key) in &modifier_keys {
            held |= bit;
            self.page_send("Input.dispatchKeyEvent", key_event("rawKeyDown", held, key))
                .await?;
        }
        // 2. 主键：可打印字符用 keyDown 产生输入，功能键用 rawKeyDown
        let kind = if chord.key.text.is_some() {
            "keyDown"
        } else {
            "rawKeyDown"
        };
        self.page_send(
            "Input.dispatchKeyEvent",
            key_event(kind, chord.modifiers, &chord.key),
        )
        .await?;
        self.page_send(
            "Input.dispatchKeyEvent",
            key_event("keyUp", chord.modifiers, &chord.key),
        )
        .await?;
        // 3. 逆序抬起修饰键
        for (bit, key) in modifier_keys.iter().rev() {
            held &= !bit;
            self.page_send("Input.dispatchKeyEvent", key_event("keyUp", held, key))
                .await?;
        }
        Ok(())
    }

    /// 【内置浏览器】【面板按键】转发面板捕获的原始键盘事件。
    /// @param input 为面板键盘事件
    /// @returns 操作结果
    pub(crate) async fn dispatch_key_input(&self, input: &KeyInput) -> Result<()> {
        let definition = KeyDefinition {
            key: input.key.clone(),
            code: input.code.clone(),
            key_code: input.key_code,
            text: input.text.clone().filter(|text| !text.is_empty()),
        };
        let kind = match input.kind.as_str() {
            "up" => "keyUp",
            _ if definition.text.is_some() => "keyDown",
            _ => "rawKeyDown",
        };
        self.page_send(
            "Input.dispatchKeyEvent",
            key_event(kind, input.modifiers, &definition),
        )
        .await?;
        Ok(())
    }

    /// 【内置浏览器】【文本插入】把整段文本插入当前焦点，支持中文等非键盘字符。
    /// @param text 为待插入文本
    /// @returns 操作结果
    pub(crate) async fn insert_text(&self, text: &str) -> Result<()> {
        self.page_send("Input.insertText", json!({ "text": text }))
            .await?;
        Ok(())
    }
}

/// 【内置浏览器】【修饰键定义】按位展开修饰键的按键定义。
/// @param modifiers 为修饰键位
/// @returns (修饰位, 按键定义) 列表
fn modifier_definitions(modifiers: i64) -> Vec<(i64, KeyDefinition)> {
    [
        (MOD_CTRL, "Control", "ControlLeft", 17),
        (MOD_ALT, "Alt", "AltLeft", 18),
        (MOD_META, "Meta", "MetaLeft", 91),
        (MOD_SHIFT, "Shift", "ShiftLeft", 16),
    ]
    .into_iter()
    .filter(|(bit, ..)| modifiers & bit != 0)
    .map(|(bit, key, code, key_code)| {
        (
            bit,
            KeyDefinition {
                key: key.to_string(),
                code: code.to_string(),
                key_code,
                text: None,
            },
        )
    })
    .collect()
}
