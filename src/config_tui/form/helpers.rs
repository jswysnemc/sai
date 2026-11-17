//! 表单辅助工具：外部 `$EDITOR` 调用、枚举选择菜单、供应商模型候选构造及类型解析。

use super::{run_form, Field};
use crate::config::AppConfig;
use crate::config_tui::mouse::{ListMouseState, MenuInput};
use crate::config_tui::ui::draw_menu_interactive;
use crate::default_models::{OPENCODE_DEFAULT_VISION_MODEL, OPENCODE_PROVIDER_ID};
use crate::i18n::text as t;
use anyhow::{bail, Result};
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{DisableBracketedPaste, EnableBracketedPaste, KeyCode};
use crossterm::execute;
use crossterm::terminal::{self, Clear, ClearType};
use std::io::{self, Write};

/// 【配置表单】【添加自定义模型】通过统一单字段表单录入自定义模型标识。
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
        t(" ADD CUSTOM MODEL ", " 添加自定义模型 "),
        &mut fields,
    )? {
        return Ok(None);
    }
    let model = fields[0].value.trim().to_string();
    Ok((!model.is_empty()).then_some(model))
}

/// 【配置表单】【枚举弹窗选择】候选项较多时打开支持鼠标点击的列表菜单。
///
/// 参数:
/// - `stdout`: 终端输出流
/// - `label`: 选项字段标题
/// - `current`: 当前生效值
/// - `choices`: 候选值列表
/// - `empty_label`: 空字符串候选项显示标签
///
/// 返回:
/// - 用户选择的候选值（取消时保留原值）
pub(super) fn select_choice(
    stdout: &mut io::Stdout,
    label: &str,
    current: &str,
    choices: &[String],
    empty_label: &'static str,
) -> Result<String> {
    let mut selected = choices.iter().position(|item| item == current).unwrap_or(0);
    let mut mouse = ListMouseState::default();
    loop {
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
        let status = crate::config_tui::theme::help_line(&[
            ("↑↓", t("move", "移动")),
            ("Enter", t("select", "选择")),
            ("q", t("keep current", "保持当前")),
        ]);
        let hitbox =
            draw_menu_interactive(stdout, label, &options, &[], selected, &mouse, &status, "")?;
        let code = match hitbox.read(&mut mouse, selected, true)? {
            MenuInput::Redraw | MenuInput::Paste(_) => continue,
            MenuInput::Select(index) | MenuInput::RightClick(index) => {
                selected = index;
                continue;
            }
            MenuInput::Activate(index) => return Ok(choices[index].clone()),
            MenuInput::Key(key) => key.code,
            MenuInput::Action(code) => code,
        };
        match code {
            KeyCode::Esc | KeyCode::Char('q') => return Ok(current.to_string()),
            KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => selected = (selected + 1).min(choices.len() - 1),
            KeyCode::Enter => return Ok(choices[selected].clone()),
            _ => {}
        }
    }
}

/// 【配置表单】【选项格式化】将空选项或 `provider\tmodel` 复合值转换为可读标签。
pub(super) fn choice_label(choice: &str, empty_label: &str) -> String {
    if choice.is_empty() {
        empty_label.to_string()
    } else if let Some((provider, model)) = choice.split_once('\t') {
        format!("{provider} / {model}")
    } else {
        choice.to_string()
    }
}

/// 【配置表单】【模型候选值】构造首项为空串（默认回退）的 `provider\tmodel` 候选集合。
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

/// 【配置表单】【识图模型值】提取当前视觉插件绑定的 `provider\tmodel` 复合字符串。
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

/// 【配置表单】【复合模型解析】将 `provider\tmodel` 拆回 `(provider_id, model)`。
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

/// 【配置表单】【数字校验】解析表单数字字段并在报错时附带字段名称。
pub(crate) fn parse_number_field<T: std::str::FromStr>(label: &str, value: &str) -> Result<T> {
    value
        .trim()
        .parse::<T>()
        .map_err(|_| anyhow::anyhow!("{label}: {} ({value})", t("invalid number", "无效数字")))
}

/// 【配置表单】【布尔校验】解析表单布尔字段。
pub(crate) fn parse_bool_field(value: &str) -> Result<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" | "y" | "1" | "on" | "启用" | "是" => Ok(true),
        "false" | "no" | "n" | "0" | "off" | "禁用" | "否" => Ok(false),
        value => bail!("{}: {value}", t("invalid boolean value", "无效布尔值")),
    }
}

/// 【配置表单】【外部编辑器】临时挂起鼠标捕获并离开备用屏，调用 `$EDITOR` 编辑完成后恢复终端状态。
///
/// 参数:
/// - `stdout`: 终端输出流
/// - `value`: 待编辑的多行文本
///
/// 返回:
/// - 终端恢复与编辑执行结果
pub(crate) fn edit_textarea(stdout: &mut io::Stdout, value: &mut String) -> Result<()> {
    // 1. 【配置表单】【鼠标挂起】用 MouseCapture::suspend_for 包裹外部编辑器生命周期，避免编辑器内点击乱码或退出后鼠标失灵
    let result = crate::cli::mouse_capture::MouseCapture::suspend_for(
        || -> Result<Result<Option<String>>> {
            // 2. 【配置表单】【退出备用屏】关闭括号粘贴、显示光标并回到主屏标准模式运行编辑器
            execute!(stdout, DisableBracketedPaste, Show)?;
            crate::cli::alternate_screen::leave_alternate_screen(stdout)?;
            execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
            stdout.flush()?;
            terminal::disable_raw_mode()?;
            crate::config_tui::ime::enable_for_edit();

            let editor_result = edit_textarea_with_editor(value);
            crate::config_tui::ime::disable_for_nav();

            // 3. 【配置表单】【恢复备用屏】无论编辑成败均重开原始模式、重进备用屏并恢复括号粘贴与隐藏光标
            let raw_res = terminal::enable_raw_mode();
            let screen_res = crate::cli::alternate_screen::enter_alternate_screen(stdout);
            let ui_res = execute!(stdout, EnableBracketedPaste, Clear(ClearType::All), Hide);
            raw_res?;
            screen_res?;
            ui_res?;
            Ok(editor_result)
        },
    )?;
    match result {
        Ok(Some(edited)) => *value = edited,
        Ok(None) => {}
        Err(err) => crate::config_tui::ui::message(stdout, &err.to_string())?,
    }
    Ok(())
}

/// 【配置表单】【进程执行】写入临时文件并调用系统外部编辑器等待用户保存退出。
fn edit_textarea_with_editor(value: &str) -> Result<Option<String>> {
    let mut file = tempfile::NamedTempFile::new()?;
    file.write_all(value.as_bytes())?;
    file.flush()?;
    let path = file.path().to_path_buf();
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
