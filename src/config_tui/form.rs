use crate::config::AppConfig;
use crate::default_models::{OPENCODE_DEFAULT_VISION_MODEL, OPENCODE_PROVIDER_ID};
use crate::i18n::text as t;
use anyhow::{bail, Result};
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{KeyCode, KeyModifiers};
use crossterm::style::{Attribute, Print, SetAttribute};
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, queue};
use std::io::{self, Write};
use std::process::Command;

use super::input::{read_key, read_key_event};
use super::layout::panel_width;
use super::ui::{display_width, draw_box, draw_menu, pad, truncate};

struct FcitxState {
    // 进入表单前输入法是否处于激活状态，退出时按此恢复
    was_active: bool,
    last_state: Option<char>,
}

impl FcitxState {
    /// 记录输入法当前状态并在导航期间临时关闭输入法。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 携带初始状态的输入法管理器
    pub(crate) fn new() -> Self {
        // 1. 查询进入表单前的输入法状态，输出 2 表示激活
        let initial = fcitx5_state();
        let was_active = initial == Some('2');
        // 2. 仅在激活时关闭，避免导航按键被输入法拦截
        if was_active {
            run_fcitx5_remote("-c");
        }
        Self {
            was_active,
            last_state: initial,
        }
    }

    fn enter_editing(&mut self) {
        if self.last_state == Some('2') {
            run_fcitx5_remote("-o");
        }
    }

    fn leave_editing(&mut self) {
        self.last_state = fcitx5_state();
        run_fcitx5_remote("-c");
    }
}

impl Drop for FcitxState {
    fn drop(&mut self) {
        // 退出表单时恢复进入前的输入法激活状态
        if self.was_active {
            run_fcitx5_remote("-o");
        }
    }
}

/// 查询 fcitx5 输入法当前状态。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 状态字符，2 表示激活；命令不可用时返回空
fn fcitx5_state() -> Option<char> {
    let output = Command::new("fcitx5-remote").output().ok()?;
    output.stdout.first().copied().map(char::from)
}

/// 同步执行 fcitx5-remote 子命令。
///
/// 参数:
/// - `arg`: fcitx5-remote 参数
///
/// 返回:
/// - 无；同步等待命令结束，避免遗留僵尸进程
fn run_fcitx5_remote(arg: &str) {
    let _ = Command::new("fcitx5-remote").arg(arg).output();
}

pub(crate) fn run_form(stdout: &mut io::Stdout, title: &str, fields: &mut [Field]) -> Result<bool> {
    let mut selected = 0usize;
    let mut editing = false;
    let mut fcitx = FcitxState::new();
    let mut cursors = fields
        .iter()
        .map(|field| field.value.chars().count())
        .collect::<Vec<_>>();
    let mut revealed_secrets = vec![false; fields.len()];
    loop {
        draw_form(
            stdout,
            title,
            fields,
            selected,
            editing,
            &cursors,
            &revealed_secrets,
        )?;
        let key = read_key_event()?;
        match key.code {
            KeyCode::Esc if editing => {
                fcitx.leave_editing();
                editing = false;
            }
            KeyCode::Esc | KeyCode::Char('q') if !editing => return Ok(false),
            KeyCode::Enter if editing => {
                fcitx.leave_editing();
                editing = false;
            }
            KeyCode::Enter if !editing && selected == fields.len() => return Ok(true),
            KeyCode::Enter if !editing && selected == fields.len() + 1 => return Ok(false),
            KeyCode::Enter if !editing && fields[selected].boolean => {
                let value = parse_bool_field(&fields[selected].value)?;
                fields[selected].value = (!value).to_string();
                cursors[selected] = fields[selected].value.chars().count();
            }
            KeyCode::Enter if !editing && !fields[selected].choices.is_empty() => {
                fields[selected].value = select_choice(
                    stdout,
                    fields[selected].label,
                    &fields[selected].value,
                    &fields[selected].choices,
                    fields[selected].empty_choice_label,
                )?;
                cursors[selected] = fields[selected].value.chars().count();
            }
            KeyCode::Enter if !editing && fields[selected].textarea => {
                // 外部编辑器返回后回到表单继续编辑，由用户显式选择保存或取消
                edit_textarea(stdout, &mut fields[selected].value)?;
                cursors[selected] = fields[selected].value.chars().count();
            }
            KeyCode::Enter if !editing => {
                if !fields[selected].boolean {
                    fcitx.enter_editing();
                    editing = true;
                }
            }
            KeyCode::Char('s') if !editing => return Ok(true),
            KeyCode::Up | KeyCode::Char('k') if !editing => selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') if !editing => {
                selected = (selected + 1).min(fields.len() + 1)
            }
            KeyCode::Left | KeyCode::Char('h') if !editing && selected == fields.len() + 1 => {
                selected = fields.len()
            }
            KeyCode::Right | KeyCode::Char('l') if !editing && selected == fields.len() => {
                selected = fields.len() + 1
            }
            KeyCode::Left if editing => cursors[selected] = cursors[selected].saturating_sub(1),
            KeyCode::Right if editing => {
                cursors[selected] =
                    (cursors[selected] + 1).min(fields[selected].value.chars().count())
            }
            KeyCode::Home if editing => cursors[selected] = 0,
            KeyCode::End if editing => cursors[selected] = fields[selected].value.chars().count(),
            KeyCode::Backspace if editing => {
                if cursors[selected] > 0 {
                    remove_char_before_cursor(&mut fields[selected].value, &mut cursors[selected]);
                }
            }
            KeyCode::Delete if editing => {
                remove_char_at_cursor(&mut fields[selected].value, cursors[selected])
            }
            KeyCode::Char('r')
                if editing
                    && fields[selected].secret
                    && key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                revealed_secrets[selected] = !revealed_secrets[selected];
            }
            KeyCode::Char(char) if editing && !key.modifiers.contains(KeyModifiers::CONTROL) => {
                insert_char_at_cursor(&mut fields[selected].value, &mut cursors[selected], char)
            }
            _ => {}
        }
    }
}

/// 通过统一表单输入一个自定义模型标识。
///
/// 参数:
/// - `stdout`: TUI 输出句柄
///
/// 返回:
/// - 非空模型标识，取消时返回空
pub(crate) fn add_custom_model_form(stdout: &mut io::Stdout) -> Result<Option<String>> {
    let mut fields = [Field::new("Model ID", String::new())];
    if !run_form(stdout, " ADD CUSTOM MODEL ", &mut fields)? {
        return Ok(None);
    }
    let model = fields[0].value.trim().to_string();
    Ok((!model.is_empty()).then_some(model))
}

fn select_choice(
    stdout: &mut io::Stdout,
    label: &str,
    current: &str,
    choices: &[String],
    empty_label: &'static str,
) -> Result<String> {
    let mut selected = choices.iter().position(|item| item == current).unwrap_or(0);
    loop {
        let options = choices
            .iter()
            .map(|choice| choice_label(choice, empty_label))
            .collect::<Vec<_>>();
        draw_menu(stdout, label, &options, selected, "")?;
        match read_key()? {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(current.to_string()),
            KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => selected = (selected + 1).min(choices.len() - 1),
            KeyCode::Enter => return Ok(choices[selected].clone()),
            _ => {}
        }
    }
}

fn choice_label(choice: &str, empty_label: &str) -> String {
    if choice.is_empty() {
        empty_label.to_string()
    } else if let Some((provider, model)) = choice.split_once('\t') {
        format!("{provider} / {model}")
    } else {
        choice.to_string()
    }
}

pub(crate) fn provider_model_choice_values(
    config: &AppConfig,
    include_current: bool,
) -> Vec<String> {
    let mut choices = vec![String::new()];
    if include_current {
        choices.push(format!(
            "{OPENCODE_PROVIDER_ID}\t{OPENCODE_DEFAULT_VISION_MODEL}"
        ));
    }
    choices.extend(
        config
            .provider_model_choices()
            .into_iter()
            .map(|choice| choice.value()),
    );
    choices
}

pub(crate) fn vision_provider_value(config: &AppConfig) -> String {
    let vision = &config.plugins.vision;
    if vision.vision_provider_id.trim().is_empty() {
        format!("{OPENCODE_PROVIDER_ID}\t{OPENCODE_DEFAULT_VISION_MODEL}")
    } else if vision.vision_model.trim().is_empty() {
        config
            .provider(Some(vision.vision_provider_id.trim()))
            .map(|provider| format!("{}\t{}", provider.id, provider.default_model))
            .unwrap_or_else(|_| vision.vision_provider_id.clone())
    } else {
        format!("{}\t{}", vision.vision_provider_id, vision.vision_model)
    }
}

pub(crate) fn kb_embedding_provider_value(config: &AppConfig) -> String {
    let kb = &config.plugins.knowledge_base;
    if kb.embedding_provider_id.trim().is_empty() {
        String::new()
    } else if kb.embedding_model.trim().is_empty() {
        config
            .provider(Some(kb.embedding_provider_id.trim()))
            .map(|provider| format!("{}\t{}", provider.id, provider.default_model))
            .unwrap_or_else(|_| kb.embedding_provider_id.clone())
    } else {
        format!("{}\t{}", kb.embedding_provider_id, kb.embedding_model)
    }
}

pub(crate) fn parse_provider_model_choice(value: &str) -> (String, String) {
    let value = value.trim();
    if value.is_empty() {
        return (String::new(), String::new());
    }
    if let Some((provider, model)) = value.split_once('\t') {
        return (provider.trim().to_string(), model.trim().to_string());
    }
    (value.to_string(), String::new())
}

/// 解析表单数字字段。
///
/// 参数:
/// - `label`: 字段标签，用于组装错误提示
/// - `value`: 字段文本值
///
/// 返回:
/// - 解析后的数字；解析失败时返回带字段名的错误
pub(crate) fn parse_number_field<T: std::str::FromStr>(label: &str, value: &str) -> Result<T> {
    value
        .trim()
        .parse::<T>()
        .map_err(|_| anyhow::anyhow!("{label}: {} ({value})", t("invalid number", "无效数字")))
}

pub(crate) fn parse_bool_field(value: &str) -> Result<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" | "y" | "1" | "on" | "启用" | "是" => Ok(true),
        "false" | "no" | "n" | "0" | "off" | "禁用" | "否" => Ok(false),
        value => bail!("{}: {value}", t("invalid boolean value", "无效布尔值")),
    }
}

fn edit_textarea(stdout: &mut io::Stdout, value: &mut String) -> Result<()> {
    execute!(
        stdout,
        Show,
        LeaveAlternateScreen,
        Clear(ClearType::All),
        MoveTo(0, 0)
    )?;
    stdout.flush()?;
    terminal::disable_raw_mode()?;
    let mut file = tempfile::NamedTempFile::new()?;
    file.write_all(value.as_bytes())?;
    let path = file.path().to_path_buf();
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_string());
    let status = Command::new(&editor)
        .arg(&path)
        .status()
        .or_else(|_| Command::new("nano").arg(&path).status());
    if let Err(err) = status {
        eprintln!("{}: {err}", t("failed to open editor", "无法打开编辑器"));
    }
    *value = std::fs::read_to_string(&path)?.trim().to_string();
    terminal::enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen, Clear(ClearType::All), Hide)?;
    Ok(())
}

fn draw_form(
    stdout: &mut io::Stdout,
    title: &str,
    fields: &[Field],
    selected: usize,
    editing: bool,
    cursors: &[usize],
    revealed_secrets: &[bool],
) -> Result<()> {
    let (cols, rows) = terminal::size()?;
    let width = panel_width(96, 48, cols.saturating_sub(8));
    let height = (fields.len() as u16 + 8)
        .min(rows.saturating_sub(4))
        .max(10);
    let x = cols.saturating_sub(width) / 2;
    let y = rows.saturating_sub(height) / 2;
    queue!(stdout, Clear(ClearType::All))?;
    draw_box(stdout, x, y, width, height, title)?;
    queue!(
        stdout,
        MoveTo(x + 2, y + 1),
        Print(t(
            "[j/k] move [Enter] edit/toggle/open editor [Ctrl+R] reveal secret [s] confirm [q] cancel",
            "[j/k]移动 [Enter]编辑/勾选/打开编辑器 [Ctrl+R]显示密钥 [s]确认 [q]取消",
        ))
    )?;
    let mut cursor = None;
    for (index, field) in fields.iter().enumerate() {
        let row_y = y + index as u16 + 3;
        queue!(stdout, MoveTo(x + 2, row_y))?;
        let marker = if index == selected { ">" } else { " " };
        let value = field_display_value(field, revealed_secrets[index]);
        let prefix = format!("{marker} {}: ", field.label);
        let line = truncate(
            &format!("{prefix}{value}"),
            width.saturating_sub(4) as usize,
        );
        if index == selected && !editing {
            queue!(
                stdout,
                SetAttribute(Attribute::Reverse),
                Print(pad(&line, width.saturating_sub(4) as usize)),
                SetAttribute(Attribute::Reset)
            )?;
        } else {
            queue!(stdout, Print(pad(&line, width.saturating_sub(4) as usize)))?;
        }
        if index == selected && editing {
            let rendered_value = rendered_text_value(field, revealed_secrets[index]);
            let cursor_text = take_chars(&rendered_value, cursors[index]);
            let cursor_x = x
                + 2
                + display_width(&prefix) as u16
                + display_width(&truncate(&cursor_text, width.saturating_sub(4) as usize)) as u16;
            cursor = Some((cursor_x.min(x + width.saturating_sub(3)), row_y));
        }
    }
    let button_y = y + fields.len() as u16 + 4;
    draw_form_button(
        stdout,
        x + 2,
        button_y,
        t(" Save ", " 保存 "),
        selected == fields.len() && !editing,
    )?;
    draw_form_button(
        stdout,
        x + 14,
        button_y,
        t(" Cancel ", " 取消 "),
        selected == fields.len() + 1 && !editing,
    )?;

    let mode = if editing && fields[selected].secret && revealed_secrets[selected] {
        t(
            "Editing secret in plain text, Ctrl+R hides it",
            "正在明文编辑密钥，Ctrl+R 隐藏",
        )
    } else if editing && fields[selected].secret {
        t(
            "Editing secret masked, Ctrl+R reveals it",
            "正在掩码编辑密钥，Ctrl+R 显示明文",
        )
    } else if editing {
        t(
            "Editing, Enter/Esc ends editing",
            "编辑中，Enter/Esc 结束编辑",
        )
    } else {
        t(
            "Navigating, Enter selects current item",
            "导航中，Enter 选择当前项",
        )
    };
    queue!(
        stdout,
        MoveTo(x + 2, y + height.saturating_sub(1)),
        Print(truncate(mode, width.saturating_sub(4) as usize))
    )?;
    if let Some((x, y)) = cursor {
        queue!(stdout, Show, MoveTo(x, y))?;
    } else {
        queue!(stdout, Hide)?;
    }
    stdout.flush()?;
    Ok(())
}

/// 返回表单字段展示文本。
///
/// 参数:
/// - `field`: 表单字段
/// - `revealed_secret`: 是否显示密钥明文
///
/// 返回:
/// - 字段展示文本
fn field_display_value(field: &Field, revealed_secret: bool) -> String {
    if field.boolean {
        match parse_bool_field(&field.value) {
            Ok(true) => "[x]".to_string(),
            Ok(false) => "[ ]".to_string(),
            Err(_) => rendered_text_value(field, revealed_secret),
        }
    } else if field.textarea && field.value.is_empty() {
        t("(Enter opens $EDITOR)", "(Enter 打开 $EDITOR)").to_string()
    } else if !field.choices.is_empty() && field.value.is_empty() {
        field.empty_choice_label.to_string()
    } else if !field.choices.is_empty() {
        choice_label(&field.value, field.empty_choice_label)
    } else {
        truncate(&rendered_text_value(field, revealed_secret), 70)
    }
}

/// 返回单行文本字段渲染值。
///
/// 参数:
/// - `field`: 表单字段
/// - `revealed_secret`: 是否显示密钥明文
///
/// 返回:
/// - 字段单行渲染值
fn rendered_text_value(field: &Field, revealed_secret: bool) -> String {
    let value = field.value.replace('\n', " ");
    if field.secret && !revealed_secret {
        mask_secret(&value)
    } else {
        value
    }
}

/// 掩码密钥文本。
///
/// 参数:
/// - `value`: 原始文本
///
/// 返回:
/// - 掩码后文本
fn mask_secret(value: &str) -> String {
    "*".repeat(value.chars().count())
}

fn draw_form_button(
    stdout: &mut io::Stdout,
    x: u16,
    y: u16,
    label: &str,
    selected: bool,
) -> Result<()> {
    queue!(stdout, MoveTo(x, y))?;
    if selected {
        queue!(
            stdout,
            SetAttribute(Attribute::Reverse),
            Print(label),
            SetAttribute(Attribute::Reset)
        )?;
    } else {
        queue!(stdout, Print(label))?;
    }
    Ok(())
}

fn insert_char_at_cursor(value: &mut String, cursor: &mut usize, ch: char) {
    let byte_index = byte_index_for_char(value, *cursor);
    value.insert(byte_index, ch);
    *cursor += 1;
}

fn remove_char_before_cursor(value: &mut String, cursor: &mut usize) {
    let end = byte_index_for_char(value, *cursor);
    let start = byte_index_for_char(value, cursor.saturating_sub(1));
    value.replace_range(start..end, "");
    *cursor -= 1;
}

fn remove_char_at_cursor(value: &mut String, cursor: usize) {
    if cursor >= value.chars().count() {
        return;
    }
    let start = byte_index_for_char(value, cursor);
    let end = byte_index_for_char(value, cursor + 1);
    value.replace_range(start..end, "");
}

fn byte_index_for_char(value: &str, char_index: usize) -> usize {
    value
        .char_indices()
        .nth(char_index)
        .map(|(index, _)| index)
        .unwrap_or(value.len())
}

fn take_chars(value: &str, count: usize) -> String {
    value.chars().take(count).collect()
}

pub(crate) struct Field {
    pub(crate) label: &'static str,
    pub(crate) value: String,
    pub(crate) textarea: bool,
    pub(crate) boolean: bool,
    pub(crate) secret: bool,
    pub(crate) choices: Vec<String>,
    pub(crate) empty_choice_label: &'static str,
}

impl Field {
    pub(crate) fn new(label: &'static str, value: String) -> Self {
        Self {
            label,
            value,
            textarea: false,
            boolean: false,
            secret: false,
            choices: Vec::new(),
            empty_choice_label: t("Use current Provider", "使用当前 Provider"),
        }
    }

    pub(crate) fn boolean(label: &'static str, value: bool) -> Self {
        Self {
            label,
            value: value.to_string(),
            textarea: false,
            boolean: true,
            secret: false,
            choices: Vec::new(),
            empty_choice_label: t("Use current Provider", "使用当前 Provider"),
        }
    }

    pub(crate) fn textarea(label: &'static str, value: String) -> Self {
        Self {
            label,
            value,
            textarea: true,
            boolean: false,
            secret: false,
            choices: Vec::new(),
            empty_choice_label: t("Use current Provider", "使用当前 Provider"),
        }
    }

    pub(crate) fn secret(mut self) -> Self {
        self.secret = true;
        self
    }

    pub(crate) fn choices(mut self, choices: &[&str]) -> Self {
        self.choices = choices.iter().map(|item| item.to_string()).collect();
        self
    }

    pub(crate) fn choices_owned(mut self, choices: Vec<String>) -> Self {
        self.choices = choices;
        self
    }

    pub(crate) fn empty_choice_label(mut self, label: &'static str) -> Self {
        self.empty_choice_label = label;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_field_is_masked_by_default() {
        let field = Field::new("Token", "secret".to_string()).secret();

        assert_eq!(field_display_value(&field, false), "******");
    }

    #[test]
    fn secret_field_can_be_revealed() {
        let field = Field::new("Token", "secret".to_string()).secret();

        assert_eq!(field_display_value(&field, true), "secret");
    }

    #[test]
    fn secret_textarea_is_masked_by_default() {
        let field = Field::textarea("Tokens", "first\nsecond".to_string()).secret();

        assert_eq!(field_display_value(&field, false), "************");
    }
}
