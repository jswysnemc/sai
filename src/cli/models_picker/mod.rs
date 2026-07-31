mod render;
mod state;

use crate::config::AppConfig;
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::{bail, Result};
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::terminal::{self, Clear, ClearType};
use crossterm::{execute, queue};
use state::PickerState;
use std::io::{self, IsTerminal, Write};

/// 可选思考等级，与 `/thinking` 命令保持一致。
const THINKING_LEVELS: &[&str] = &["auto", "none", "low", "medium", "high", "xhigh", "max"];

/// 【CLI】【模型选择】运行交互式模型与思考等级选择。
///
/// 上下键在当前列内移动，左右键切换模型列与思考列，Enter 保存，Esc 取消。
///
/// 参数:
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 保存成功或用户取消时为 Ok
pub(super) fn run(paths: &SaiPaths) -> Result<()> {
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
        println!("{}", t("cancelled", "已取消"));
        return Ok(());
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
    println!(
        "{}: {} · {}: {}",
        t("model", "模型"),
        selected.label(),
        t("thinking", "思考"),
        level
    );
    Ok(())
}

/// 运行按键循环。
///
/// 参数:
/// - `picker`: 选择状态
/// 返回:
/// - 确认时返回 Some(())，取消时返回 None
fn run_loop(picker: &mut PickerState) -> Result<Option<()>> {
    struct RawGuard;
    impl Drop for RawGuard {
        fn drop(&mut self) {
            let _ = terminal::disable_raw_mode();
            let _ = execute!(io::stdout(), Show);
        }
    }

    let mut stdout = io::stdout();
    execute!(stdout, Hide)?;
    let _guard = RawGuard;
    let mut previous_rows = 0usize;
    loop {
        let lines = render::render(picker);
        draw(&mut stdout, &lines, previous_rows)?;
        previous_rows = lines.len();

        terminal::enable_raw_mode()?;
        let key = read_key();
        terminal::disable_raw_mode()?;
        let key = key?;
        match key.code {
            KeyCode::Esc => return Ok(None),
            KeyCode::Enter if picker.selected_model().is_some() => return Ok(Some(())),
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

/// 重绘选择界面。
///
/// 参数:
/// - `stdout`: 标准输出
/// - `lines`: 待绘制的行
/// - `previous_rows`: 上一帧行数，用于回退光标覆盖重绘
///
/// 返回:
/// - 绘制结果
fn draw(stdout: &mut io::Stdout, lines: &[String], previous_rows: usize) -> Result<()> {
    if previous_rows > 0 {
        queue!(stdout, crossterm::cursor::MoveUp(previous_rows as u16))?;
    }
    for line in lines {
        queue!(stdout, Clear(ClearType::CurrentLine))?;
        writeln!(stdout, "{line}")?;
    }
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
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
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
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn unknown_provider_is_reported() {
        let mut config = AppConfig::default();

        assert!(apply_level(&mut config, "missing-provider", "high").is_err());
    }

    /// 【CLI】【模型选择】验证空思考等级归一化为 auto。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn empty_level_normalizes_to_auto() {
        assert_eq!(normalized_level(""), "auto");
        assert_eq!(normalized_level("  "), "auto");
        assert_eq!(normalized_level(" high "), "high");
    }
}
