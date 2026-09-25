mod field;
mod render;

pub(crate) use field::Field;
use render::draw_form;

use crate::config::AppConfig;
use crate::default_models::{OPENCODE_DEFAULT_VISION_MODEL, OPENCODE_PROVIDER_ID};
use crate::i18n::text as t;
use anyhow::{bail, Result};
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{KeyCode, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use std::io::{self, Write};

use super::input::{read_key, read_key_event};
use super::ui::draw_menu;

struct FcitxState;

impl FcitxState {
    /// 表单导航使用会话级输入法状态：列表里保持关闭，进入文本框再打开。
    fn new() -> Self {
        super::ime::disable_for_nav();
        Self
    }

    fn enter_editing(&mut self) {
        super::ime::enable_for_edit();
    }

    fn leave_editing(&mut self) {
        super::ime::disable_for_nav();
    }
}

impl Drop for FcitxState {
    fn drop(&mut self) {
        super::ime::disable_for_nav();
    }
}

pub(crate) fn run_form(stdout: &mut io::Stdout, title: &str, fields: &mut [Field]) -> Result<bool> {
    let mut selected = first_selectable(fields);
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
            KeyCode::Up | KeyCode::Char('k') if !editing => {
                selected = previous_selectable(fields, selected)
            }
            KeyCode::Down | KeyCode::Char('j') if !editing => {
                selected = next_selectable(fields, selected)
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

/// 返回第一个可选中（非分组标题）的字段下标。
fn first_selectable(fields: &[Field]) -> usize {
    fields.iter().position(|field| !field.section).unwrap_or(0)
}

/// 向下移动选中项，跳过分组标题；越过末尾进入按钮区。
fn next_selectable(fields: &[Field], selected: usize) -> usize {
    let mut next = selected.saturating_add(1);
    while next < fields.len() && fields[next].section {
        next += 1;
    }
    next.min(fields.len() + 1)
}

/// 向上移动选中项，跳过分组标题；已在顶部时停在首个可选字段。
fn previous_selectable(fields: &[Field], selected: usize) -> usize {
    if selected == 0 {
        return first_selectable(fields);
    }
    let mut next = selected - 1;
    loop {
        if next >= fields.len() || !fields[next].section {
            return next;
        }
        if next == 0 {
            return first_selectable(fields);
        }
        next -= 1;
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
    let mut fields = [Field::new(t("Model ID", "模型标识"), String::new())];
    if !run_form(
        stdout,
        &t(" ADD CUSTOM MODEL ", " 添加自定义模型 "),
        &mut fields,
    )? {
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
        // 当前生效值带勾选标记，视线不必依赖记忆
        let options = choices
            .iter()
            .map(|choice| {
                let text = choice_label(choice, empty_label);
                if choice == current {
                    format!("{text} ✓")
                } else {
                    text
                }
            })
            .collect::<Vec<_>>();
        let status = super::theme::help_line(&[
            ("↑↓", t("move", "移动")),
            ("Enter", t("select", "选择")),
            ("q", t("keep current", "保持当前")),
        ]);
        draw_menu(stdout, label, &options, selected, &status)?;
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
    if include_current
        && config
            .providers
            .iter()
            .any(|provider| provider.id == OPENCODE_PROVIDER_ID && provider.enabled)
    {
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
        String::new()
    } else if vision.vision_model.trim().is_empty() {
        config
            .provider(Some(vision.vision_provider_id.trim()))
            .map(|provider| format!("{}\t{}", provider.id, provider.default_model))
            .unwrap_or_else(|_| vision.vision_provider_id.clone())
    } else {
        format!("{}\t{}", vision.vision_provider_id, vision.vision_model)
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

/// 临时离开备用屏，用 $EDITOR 编辑多行文本后返回。
pub(super) fn edit_textarea(stdout: &mut io::Stdout, value: &mut String) -> Result<()> {
    execute!(
        stdout,
        Show,
        LeaveAlternateScreen,
        Clear(ClearType::All),
        MoveTo(0, 0)
    )?;
    stdout.flush()?;
    terminal::disable_raw_mode()?;
    super::ime::enable_for_edit();
    let result = edit_textarea_with_editor(value);
    super::ime::disable_for_nav();
    // 无论成败都要先回到备用屏：错误提示得画在界面里才看得见，
    // 之前 eprintln 写 stderr，而备用屏占着显示，用户只会看到「什么都没发生」
    terminal::enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen, Clear(ClearType::All), Hide)?;
    match result {
        Ok(Some(edited)) => *value = edited,
        // 编辑器以非零码退出（如 vim 的 :cq）视为取消，保留原值
        Ok(None) => {}
        Err(err) => super::ui::message(stdout, &err.to_string())?,
    }
    Ok(())
}

/// 调用外部编辑器编辑多行文本。
///
/// 参数:
/// - `value`: 待编辑的原文
///
/// 返回:
/// - `Ok(Some(...))` 为编辑后的内容；`Ok(None)` 表示用户取消；`Err` 为无法编辑
fn edit_textarea_with_editor(value: &str) -> Result<Option<String>> {
    let mut file = tempfile::NamedTempFile::new()?;
    file.write_all(value.as_bytes())?;
    file.flush()?;
    let path = file.path().to_path_buf();
    // 编辑器与启动方式都交给平台层：Windows 上没有 vim/nano，
    // 且 GUI 编辑器要直接启动才能等到窗口关闭
    let editor = std::env::var("EDITOR")
        .unwrap_or_else(|_| crate::platform::shell::default_editor().to_string());
    let status = crate::platform::shell::editor_command(&editor, &path)
        .status()
        .map_err(|err| {
            anyhow::anyhow!("{}: {err}", t("failed to open editor", "无法打开编辑器"))
        })?;
    if !status.success() {
        return Ok(None);
    }
    Ok(Some(std::fs::read_to_string(&path)?.trim().to_string()))
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 未指定识图模型时保持空选项，避免保存时把已停用的 opencode 写进去。
    #[test]
    fn empty_vision_provider_stays_empty() {
        let config = AppConfig::default();
        assert!(vision_provider_value(&config).is_empty());
    }

    /// 分组标题行不参与导航：上下移动会跳过它。
    #[test]
    fn navigation_skips_section_rows() {
        let fields = vec![
            Field::section("组一"),
            Field::new("A", String::new()),
            Field::section("组二"),
            Field::new("B", String::new()),
        ];

        assert_eq!(first_selectable(&fields), 1);
        assert_eq!(next_selectable(&fields, 1), 3);
        assert_eq!(previous_selectable(&fields, 3), 1);
        // 首字段再向上仍停在首个可选字段
        assert_eq!(previous_selectable(&fields, 1), 1);
        // 末字段向下进入按钮区
        assert_eq!(next_selectable(&fields, 3), 4);
    }
}
