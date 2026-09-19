mod draft;
#[cfg(test)]
mod tests;
mod view;

use crate::config::AppConfig;
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use crate::state::{CompactionBudgetPolicy, StateStore};
use anyhow::Result;
use crossterm::{
    cursor::{Hide, Show},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use draft::{Draft, Outcome};
use std::{io, time::Duration};
use view::PreviewContext;

/// 【上下文】【全局设置】编辑全局策略，保存后更新调用方配置
/// 参数: stdout 为终端输出，config 为内存配置
/// 返回: 交互结果；取消不改变配置，落盘沿用主配置界面
pub(super) fn edit_defaults(stdout: &mut io::Stdout, config: &mut AppConfig) -> Result<()> {
    let policy = CompactionBudgetPolicy::from_context(
        config.context.compaction_ratio,
        config.context.compaction_reserve_tokens,
    );
    let context = PreviewContext {
        window: config.active_context_window_tokens().unwrap_or(0),
        used: None,
        scope: t(
            "Workspace defaults · active model preview",
            "全局默认 · 当前模型窗口预览",
        )
        .to_string(),
        session: false,
    };
    let next = match edit(stdout, &context, policy, CompactionBudgetPolicy::DEFAULT)? {
        Outcome::Save(next) => next,
        Outcome::Reset => CompactionBudgetPolicy::DEFAULT,
        Outcome::Cancel => return Ok(()),
    };
    config.context.compaction_ratio = next.ratio;
    config.context.compaction_reserve_tokens = next.reserve_tokens;
    Ok(())
}

/// 【上下文】【会话设置】打开当前会话策略面板并保存会话覆盖
/// 参数: paths 为应用存储路径
/// 返回: 保存或取消提示，外部内核直接返回说明
pub(crate) fn run_session(paths: &SaiPaths) -> Result<String> {
    let config = AppConfig::load_or_default(paths)?;
    if config.agent.engine.is_external() {
        return Ok(t(
            "Context is managed by the external engine",
            "上下文由外部内核自行管理",
        )
        .to_string());
    }
    let state = StateStore::new(paths)?;
    let current = state.resolve_compaction_policy(&config.context)?;
    let defaults = CompactionBudgetPolicy::from_context(
        config.context.compaction_ratio,
        config.context.compaction_reserve_tokens,
    );
    let snapshot =
        state.session_snapshot(config.active_context_window_tokens().unwrap_or(128_000))?;
    let context = PreviewContext {
        window: snapshot.context_window_tokens,
        used: Some(snapshot.context_prompt_tokens),
        scope: if current.session_override {
            t(
                "This session · overrides workspace defaults",
                "本会话 · 已覆盖全局默认",
            )
        } else {
            t(
                "This session · using workspace defaults",
                "本会话 · 沿用全局默认",
            )
        }
        .to_string(),
        session: true,
    };
    // 1. 【上下文】【面板交互】进入独立备用屏，通过析构保证出错时也恢复终端
    let terminal = PanelTerminal::start()?;
    let outcome = edit(&mut io::stdout(), &context, current.policy, defaults);
    drop(terminal);
    // 2. 【上下文】【面板交互】用户确认后才写入会话配置，取消与中断均不写入
    match outcome {
        Ok(Outcome::Save(policy)) => {
            state.save_compaction_policy(policy.ratio, policy.reserve_tokens)?;
            Ok(t("Session compaction policy saved", "本会话压缩策略已保存").to_string())
        }
        Ok(Outcome::Reset) => {
            state.clear_compaction_policy()?;
            Ok(t(
                "Using workspace compaction defaults",
                "已恢复全局压缩默认值",
            )
            .to_string())
        }
        Ok(Outcome::Cancel) => Ok(t("Compaction settings cancelled", "已取消压缩设置").to_string()),
        Err(error) if error.downcast_ref::<super::input::Interrupted>().is_some() => {
            Ok(t("Compaction settings cancelled", "已取消压缩设置").to_string())
        }
        Err(error) => Err(error),
    }
}

/// 【上下文】【面板交互】绘制草稿并处理输入，不直接写入配置
/// 参数: stdout 为输出，context 为窗口信息，policy/defaults 为当前与默认策略
/// 返回: 用户确认的保存、恢复或取消操作
fn edit(
    stdout: &mut io::Stdout,
    context: &PreviewContext,
    policy: CompactionBudgetPolicy,
    defaults: CompactionBudgetPolicy,
) -> Result<Outcome> {
    let mut draft = Draft::new(policy, defaults);
    let mut size = terminal::size()?;
    let mut redraw = true;
    loop {
        if redraw {
            view::draw(stdout, &draft, context)?;
        }
        redraw = false;
        if let Some(key) =
            super::input::read_key_event_with_timeout(Some(Duration::from_millis(200)))?
        {
            if let Some(outcome) = draft.handle(key.code) {
                return Ok(outcome);
            }
            redraw = true;
        }
        // 1. 【上下文】【面板交互】空闲时仅检查尺寸，避免持续清屏导致闪烁与输出积压
        let next_size = terminal::size()?;
        if next_size != size {
            size = next_size;
            redraw = true;
        }
    }
}

/// 【上下文】【终端恢复】保存进入面板前的原始输入模式
struct PanelTerminal {
    was_raw: bool,
}

impl PanelTerminal {
    /// 【上下文】【终端恢复】进入备用屏，返回用于恢复终端的守卫
    fn start() -> Result<Self> {
        let was_raw = terminal::is_raw_mode_enabled()?;
        terminal::enable_raw_mode()?;
        if let Err(error) = execute!(io::stdout(), EnterAlternateScreen, Hide) {
            if !was_raw {
                let _ = terminal::disable_raw_mode();
            }
            return Err(error.into());
        }
        Ok(Self { was_raw })
    }
}

impl Drop for PanelTerminal {
    /// 【上下文】【终端恢复】退出时恢复主屏与原始输入模式；返回: 无
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), Show, LeaveAlternateScreen);
        if !self.was_raw {
            let _ = terminal::disable_raw_mode();
        }
    }
}
