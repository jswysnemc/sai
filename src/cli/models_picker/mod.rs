mod render;
mod state;

use crate::config::AppConfig;
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::{bail, Result};
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::{execute, queue};
use state::PickerState;
use std::io::{self, IsTerminal, Write};

/// 可选思考等级，与 `/thinking` 命令保持一致。
const THINKING_LEVELS: &[&str] = &["auto", "none", "low", "medium", "high", "xhigh", "max"];

/// 交互式模型选择的结果（CLI `sai models` 与 TUI `/model` 共用）。
#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) enum PickerOutcome {
    /// 用户取消
    Cancelled,
    /// 已保存模型与思考等级
    Saved { message: String },
}

/// 【CLI】【模型选择】运行交互式模型与思考等级选择（终端打印结果）。
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
        PickerOutcome::Saved { message } => {
            println!("{message}");
            Ok(())
        }
    }
}

/// 【CLI/TUI】【模型选择】运行交互式双列选择并写回配置。
///
/// 与独立 `sai models` 相同：↑↓ 当前列移动，←→ 切换模型/思考列，
/// 输入过滤，Enter 保存，Esc 取消。可在 REPL 已启用 raw mode 时嵌套调用。
///
/// 参数:
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 取消或已保存的结果
pub(super) fn run_interactive(paths: &SaiPaths) -> Result<PickerOutcome> {
    AppConfig::init_files(paths)?;
    let mut config = AppConfig::load(paths)?;
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


    // 1. 以当前生效的供应商与模型为起点
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

    // 2. 进入交互循环
    let Some(()) = run_loop(&mut picker)? else {
        return Ok(PickerOutcome::Cancelled);
    };

    // 3. 保存所选取值
    let selected = picker
        .selected_model()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("{}", t("no matching model", "没有匹配的模型")))?;
    config.set_active_provider_model(&selected.provider_id, &selected.model)?;
    let level = picker.selected_level();
    apply_level(&mut config, &selected.provider_id, level)?;
    config.save(paths)?;
    let message = format!(
        "{}: {} · {}: {}",
        t("model", "模型"),
        selected.label(),
        t("thinking", "思考"),
        level
    );
    Ok(PickerOutcome::Saved { message })
}

/// 运行按键循环（固定锚点行绘制，兼容 CLI 与 TUI 嵌套）。
///
/// 参数:
/// - `picker`: 选择状态
///
/// 返回:
/// - 确认时返回 Some(())，取消时返回 None
fn run_loop(picker: &mut PickerState) -> Result<Option<()>> {
    let was_raw = terminal::is_raw_mode_enabled().unwrap_or(false);
    if !was_raw {
        terminal::enable_raw_mode()?;
    }
    struct ModeGuard {
        was_raw: bool,
    }
    impl Drop for ModeGuard {
        fn drop(&mut self) {
            let _ = execute!(io::stdout(), Show);
            if !self.was_raw {
                let _ = terminal::disable_raw_mode();
            }
        }
    }
    let _guard = ModeGuard { was_raw };

    let mut stdout = io::stdout();
    execute!(stdout, Hide)?;

    // 预留固定高度区域，避免在 composer 上滚动污染 transcript
    let frame_rows = render::frame_row_count();
    reserve_frame_space(frame_rows)?;
    let (_, cursor_y) = crossterm::cursor::position().unwrap_or((0, frame_rows.saturating_sub(1)));
    let anchor_y = cursor_y.saturating_sub(frame_rows.saturating_sub(1));

    loop {
        let lines = render::render(picker);
        draw_at(&mut stdout, anchor_y, frame_rows, &lines)?;

        let key = read_key()?;
        match key.code {
            KeyCode::Esc => {
                clear_frame(&mut stdout, anchor_y, frame_rows)?;
                return Ok(None);
            }
            KeyCode::Enter if picker.selected_model().is_some() => {
                clear_frame(&mut stdout, anchor_y, frame_rows)?;
                return Ok(Some(()));
            }
            KeyCode::Up => picker.move_up(),
            KeyCode::Down => picker.move_down(),
            KeyCode::Left => picker.focus_model(),
            KeyCode::Right => picker.focus_thinking(),
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

/// 预留下拉选择区域。
///
/// 参数:
/// - `rows`: 占用行数
///
/// 返回:
/// - 是否成功
fn reserve_frame_space(rows: u16) -> Result<()> {
    // 在 raw mode 下用 \r\n 推进光标，避免禁用 raw 再 println
    let mut stdout = io::stdout();
    for _ in 1..rows {
        queue!(stdout, crossterm::style::Print("\r\n"))?;
    }
    stdout.flush()?;
    Ok(())
}

/// 在固定锚点绘制一帧。
///
/// 参数:
/// - `stdout`: 标准输出
/// - `anchor_y`: 首行行号
/// - `frame_rows`: 预留总行数
/// - `lines`: 待绘制内容
///
/// 返回:
/// - 绘制结果
fn draw_at(stdout: &mut io::Stdout, anchor_y: u16, frame_rows: u16, lines: &[String]) -> Result<()> {
    for row in 0..frame_rows {
        queue!(
            stdout,
            MoveTo(0, anchor_y.saturating_add(row)),
            Clear(ClearType::CurrentLine)
        )?;
        if let Some(line) = lines.get(row as usize) {
            queue!(stdout, crossterm::style::Print(line))?;
        }
    }
    stdout.flush()?;
    Ok(())
}

/// 清空选择区域。
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

/// 读取一次按键。
///
/// 返回:
/// - 按键事件
fn read_key() -> Result<KeyEvent> {
    loop {
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Release {
                return Ok(key);
            }
        }
    }
}

/// 归一化配置中的思考等级。
///
/// 参数:
/// - `level`: 配置中的原始取值
///
/// 返回:
/// - 可选列表中的等级；空值视为 auto
fn normalized_level(level: &str) -> &str {
    let trimmed = level.trim();
    if trimmed.is_empty() {
        "auto"
    } else {
        trimmed
    }
}

/// 将思考等级写回指定供应商。
///
/// 参数:
/// - `config`: 应用配置
/// - `provider_id`: 供应商标识
/// - `level`: 目标思考等级；auto 表示交由供应商默认行为
///
/// 返回:
/// - 找到供应商时为 Ok
fn apply_level(config: &mut AppConfig, provider_id: &str, level: &str) -> Result<()> {
    let provider = config
        .providers
        .iter_mut()
        .find(|provider| provider.id == provider_id)
        .ok_or_else(|| {
            anyhow::anyhow!("{}: {provider_id}", t("unknown provider", "未知 provider"))
        })?;
    provider.thinking_level = if level == "auto" {
        String::new()
    } else {
        level.to_string()
    };
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

    /// 【CLI】【模型选择】验证未知供应商返回错误而不静默忽略。
    #[test]
    fn unknown_provider_is_reported() {
        let mut config = AppConfig::default();

        assert!(apply_level(&mut config, "missing-provider", "high").is_err());
    }

    /// 【CLI】【模型选择】验证空思考等级归一化为 auto。
    #[test]
    fn empty_level_normalizes_to_auto() {
        assert_eq!(normalized_level(""), "auto");
        assert_eq!(normalized_level("  "), "auto");
        assert_eq!(normalized_level(" high "), "high");
    }
}
