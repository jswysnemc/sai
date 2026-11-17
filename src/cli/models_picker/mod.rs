mod context;
#[cfg(test)]
mod context_tests;
mod hover_style;
mod mouse;
#[cfg(test)]
mod mouse_tests;
mod render;
mod state;
mod subagents;

use crate::config::AppConfig;
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::{bail, Result};
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{DisableBracketedPaste, EnableBracketedPaste, KeyCode, KeyModifiers};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::{execute, queue};
use mouse::PickerInput;
use state::{PickerColumn, PickerState};
use std::io::{self, IsTerminal, Write};

/// 可选思考等级，与 `/thinking` 命令保持一致。
const THINKING_LEVELS: &[&str] = &["auto", "none", "low", "medium", "high", "xhigh", "max"];

/// 交互式模型选择的结果（CLI `sai models` 与 TUI `/model` 共用）。
#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) enum PickerOutcome {
    /// 用户取消
    Cancelled,
    /// 已保存模型与思考等级
    Saved {
        message: String,
        main_model_changed: bool,
    },
}

/// 【CLI】【模型选择】运行交互式模型与思考等级选择并在终端打印结果。
///
/// 参数:
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 保存成功或用户取消时为 Ok
pub(super) fn run(paths: &SaiPaths) -> Result<()> {
    match run_interactive(paths)? {
        PickerOutcome::Cancelled => {
            println!("{}", t("cancelled", "已取消"));
            Ok(())
        }
        PickerOutcome::Saved { message, .. } => {
            println!("{message}");
            Ok(())
        }
    }
}

/// 【CLI/TUI】【模型选择】运行交互式三列选择并写回配置。
///
/// 参数:
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 取消或已保存的结果
pub(super) fn run_interactive(paths: &SaiPaths) -> Result<PickerOutcome> {
    AppConfig::init_files(paths)?;
    let mut config = AppConfig::load(paths)?;
    let initial_subagent = config.subagent.clone();
    let choices = config.provider_model_choices();
    if choices.is_empty() {
        bail!(
            "{}",
            t(
                "no configured provider models; run `sai config` first",
                "尚未配置任何模型；请先运行 `sai config`",
            )
        );
    }
    if !io::stdout().is_terminal() || !io::stdin().is_terminal() {
        bail!(
            "{}",
            t(
                "`sai models` needs an interactive terminal",
                "`sai models` 需要交互式终端",
            )
        );
    }

    // 1. 【模型选择】【起点定位】以当前生效的供应商与模型为初始选中项
    let active = config.provider(None)?;
    let current_provider = active.id.clone();
    let current_model = active.default_model.clone();
    let current_level = active.thinking_level.clone();
    let mut picker = PickerState::new(
        choices,
        THINKING_LEVELS.to_vec(),
        &current_provider,
        &current_model,
        normalized_level(&current_level),
    );
    picker.enable_context(&config);

    // 2. 【模型选择】【交互循环】支持键盘、粘贴与鼠标三列选择
    let Some(()) = run_loop(&mut picker, &mut config, paths)? else {
        if config.subagent != initial_subagent {
            return Ok(PickerOutcome::Saved {
                message: t("Subagent model settings saved", "子任务模型设置已保存").to_string(),
                main_model_changed: false,
            });
        }
        return Ok(PickerOutcome::Cancelled);
    };

    // 3. 【模型选择】【持久化保存】写回激活模型、思考等级与上下文预算
    let selected = picker
        .selected_model()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("{}", t("no matching model", "没有匹配的模型")))?;
    config.set_active_provider_model(&selected.provider_id, &selected.model)?;
    let level = picker.selected_level();
    apply_level(&mut config, &selected.provider_id, level)?;
    let context = picker
        .context_selection()
        .ok_or_else(|| anyhow::anyhow!("missing model context selection"))?;
    let limit = context.selected_limit()?;
    config
        .providers
        .iter_mut()
        .find(|provider| provider.id == selected.provider_id)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "{}: {}",
                t("unknown provider", "未知供应商"),
                selected.provider_id
            )
        })?
        .set_model_context_limit_tokens_for(&selected.model, limit);
    config.save(paths)?;
    let message = format!(
        "{}: {} · {}: {} · {}",
        t("model", "模型"),
        selected.label(),
        t("thinking", "思考"),
        level,
        context.status(),
    );
    Ok(PickerOutcome::Saved {
        message,
        main_model_changed: true,
    })
}

/// 【模型选择】【事件循环】在固定锚点绘制内联面板，响应键盘、粘贴与子面板切换。
///
/// 参数:
/// - `picker`: 选择状态
/// - `config`: 应用配置，供子智能体模型设置写回
/// - `paths`: Sai 路径集合，用于配置落盘
///
/// 返回:
/// - 确认时返回 `Some(())`，取消或中断时返回 `None`
fn run_loop(
    picker: &mut PickerState,
    config: &mut AppConfig,
    paths: &SaiPaths,
) -> Result<Option<()>> {
    let was_raw = terminal::is_raw_mode_enabled().unwrap_or(false);
    if !was_raw {
        terminal::enable_raw_mode()?;
    }
    struct ModeGuard {
        was_raw: bool,
    }
    impl Drop for ModeGuard {
        fn drop(&mut self) {
            let _ = execute!(io::stdout(), DisableBracketedPaste, Show);
            if !self.was_raw {
                let _ = terminal::disable_raw_mode();
            }
        }
    }
    let _guard = ModeGuard { was_raw };
    let _mouse = crate::cli::mouse_capture::MouseCapture::acquire()?;

    let mut stdout = io::stdout();
    execute!(stdout, EnableBracketedPaste, Hide)?;

    let mut frame = mouse::PickerFrame::new()?;
    let mut click = Default::default();
    loop {
        let lines = render::render_sized(picker, frame.width, usize::from(frame.rows), true);
        let cursor = render::input_cursor_position(picker, frame.width, usize::from(frame.rows));
        draw_at(&mut stdout, frame.anchor, frame.rows, &lines, cursor)?;
        let Some(input) = frame.read(picker, &mut click, true)? else {
            continue;
        };
        let key = match input {
            PickerInput::Paste(text) => {
                // 1. 【模型选择】【粘贴分发】上下文列粘贴数值，其余列粘贴过滤词
                if picker.column() == PickerColumn::Context {
                    if let Some(context) = picker.context_selection_mut() {
                        context.paste_input(&text);
                        continue;
                    }
                }
                picker.paste_filter(&text);
                continue;
            }
            PickerInput::Key(key) => key,
        };
        if picker.column() == PickerColumn::Context {
            if let Some(context) = picker.context_selection_mut() {
                if context.handle_key(key) {
                    continue;
                }
            }
        }
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                clear_frame(&mut stdout, frame.anchor, frame.rows)?;
                return Ok(None);
            }
            KeyCode::Esc => {
                clear_frame(&mut stdout, frame.anchor, frame.rows)?;
                return Ok(None);
            }
            KeyCode::Enter if picker.selected_model().is_some() => {
                if let Some(context) = picker.context_selection_mut() {
                    if !context.validate() {
                        while picker.column() != PickerColumn::Context {
                            picker.focus_right();
                        }
                        continue;
                    }
                }
                clear_frame(&mut stdout, frame.anchor, frame.rows)?;
                return Ok(Some(()));
            }
            KeyCode::Up => picker.move_up(),
            KeyCode::Down => picker.move_down(),
            KeyCode::Left => picker.focus_left(),
            KeyCode::Right => picker.focus_right(),
            KeyCode::Tab => {
                // 2. 【模型选择】【子任务面板】子面板按 Ctrl+C 时直接退出整个选择器
                click.clear();
                picker.set_hovered(None);
                if subagents::run(&mut stdout, &mut frame, config, paths)? {
                    return Ok(None);
                }
            }
            KeyCode::Backspace => picker.pop_filter(),
            KeyCode::Delete => picker.clear_filter(),
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                picker.clear_filter();
            }
            KeyCode::Char(ch)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                picker.push_filter(ch);
            }
            _ => {}
        }
    }
}

/// 【模型选择】【预留空间】在 raw mode 下推进下拉选择区域所需行数。
///
/// 参数:
/// - `rows`: 占用行数
///
/// 返回:
/// - 是否成功
fn reserve_frame_space(rows: u16) -> Result<()> {
    let mut stdout = io::stdout();
    for _ in 1..rows {
        queue!(stdout, crossterm::style::Print("\r\n"))?;
    }
    stdout.flush()?;
    Ok(())
}

/// 【模型选择】【锚点绘制】在固定锚点重绘一帧并按需定位显示输入光标。
///
/// 参数:
/// - `stdout`: 标准输出
/// - `anchor_y`: 首行行号
/// - `frame_rows`: 预留总行数
/// - `lines`: 待绘制内容
/// - `cursor`: 相对面板左上角的输入光标位置
///
/// 返回:
/// - 绘制结果
fn draw_at(
    stdout: &mut io::Stdout,
    anchor_y: u16,
    frame_rows: u16,
    lines: &[String],
    cursor: Option<(u16, u16)>,
) -> Result<()> {
    let width = usize::from(terminal::size()?.0.saturating_sub(1));
    queue!(stdout, Hide)?;
    for row in 0..frame_rows {
        queue!(
            stdout,
            MoveTo(0, anchor_y.saturating_add(row)),
            Clear(ClearType::CurrentLine)
        )?;
        if let Some(line) = lines.get(row as usize) {
            queue!(
                stdout,
                crossterm::style::Print(crate::render::ansi_clip::clip_ansi_to_width(
                    line, width, "..."
                ))
            )?;
        }
    }
    if let Some((col, row)) = cursor.filter(|(_, r)| *r < frame_rows) {
        queue!(
            stdout,
            MoveTo(col.min(width as u16), anchor_y.saturating_add(row)),
            Show
        )?;
    }
    stdout.flush()?;
    Ok(())
}

/// 【模型选择】【清理面板】清空选择区域并恢复光标显示。
///
/// 参数:
/// - `stdout`: 标准输出
/// - `anchor_y`: 首行行号
/// - `frame_rows`: 预留总行数
///
/// 返回:
/// - 是否成功
fn clear_frame(stdout: &mut io::Stdout, anchor_y: u16, frame_rows: u16) -> Result<()> {
    for row in 0..frame_rows {
        queue!(
            stdout,
            MoveTo(0, anchor_y.saturating_add(row)),
            Clear(ClearType::CurrentLine)
        )?;
    }
    queue!(stdout, MoveTo(0, anchor_y), Show)?;
    stdout.flush()?;
    Ok(())
}

/// 【模型选择】【等级归一】将配置中的空思考等级归一化为 auto。
///
/// 参数:
/// - `level`: 配置中的原始取值
///
/// 返回:
/// - 可选列表中的等级名称
fn normalized_level(level: &str) -> &str {
    let trimmed = level.trim();
    if trimmed.is_empty() {
        "auto"
    } else {
        trimmed
    }
}

/// 【模型选择】【写回等级】将思考等级按统一存储编码写回指定供应商。
///
/// 参数:
/// - `config`: 应用配置
/// - `provider_id`: 供应商标识
/// - `level`: 目标思考等级；auto 存为空串以保留默认行为
///
/// 返回:
/// - 找到供应商时为 Ok
fn apply_level(config: &mut AppConfig, provider_id: &str, level: &str) -> Result<()> {
    let provider = config
        .providers
        .iter_mut()
        .find(|provider| provider.id == provider_id)
        .ok_or_else(|| {
            anyhow::anyhow!("{}: {provider_id}", t("unknown provider", "未知供应商"))
        })?;
    provider.thinking_level = crate::cli::providers::stored_thinking_level(level);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【CLI】【模型选择】验证 auto 写回为空值，其余等级原样保留。
    #[test]
    fn auto_level_clears_the_provider_override() {
        let mut config = AppConfig::default();
        let provider_id = config.providers[0].id.clone();

        apply_level(&mut config, &provider_id, "high").unwrap();
        assert_eq!(config.providers[0].thinking_level, "high");

        apply_level(&mut config, &provider_id, "auto").unwrap();
        assert!(config.providers[0].thinking_level.is_empty());
    }

    /// 【CLI】【模型选择】验证未知供应商返回规范提示而不静默忽略。
    #[test]
    fn unknown_provider_is_reported() {
        let mut config = AppConfig::default();
        let err = apply_level(&mut config, "missing-provider", "high")
            .unwrap_err()
            .to_string();
        assert!(err.contains("missing-provider"));
        assert!(!err.contains("未知 provider"));
    }

    /// 【CLI】【模型选择】验证空思考等级归一化为 auto。
    #[test]
    fn empty_level_normalizes_to_auto() {
        assert_eq!(normalized_level(""), "auto");
        assert_eq!(normalized_level("  "), "auto");
        assert_eq!(normalized_level(" high "), "high");
    }
}
