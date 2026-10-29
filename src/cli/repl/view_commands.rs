use super::*;
use crate::config::TuiMode;

/// 【终端】【视图命令】解析切换参数，空参数在两种视图间切换。
/// 参数: `rest` 为命令参数，`current` 为当前视图；返回: 目标视图或用法错误。
fn requested_view(rest: &str, current: TuiMode) -> Result<TuiMode> {
    if rest.trim().is_empty() {
        return Ok(current.toggled());
    }
    TuiMode::parse(rest).ok_or_else(|| {
        anyhow::anyhow!(t(
            "usage: /view [inline|fullscreen]",
            "用法: /view [inline|fullscreen]",
        ))
    })
}

/// 【终端】【视图保存】只修改磁盘配置中的展示偏好，避免写入 Agent 运行期覆盖。
/// 参数: `paths` 为配置路径，`style` 为目标视图；返回: 保存结果。
fn save_view(paths: &SaiPaths, style: TuiMode) -> Result<()> {
    let mut stored = AppConfig::load_or_default(paths)?;
    stored.display.fullscreen = style == TuiMode::Fullscreen;
    stored.save(paths)
}

/// 【终端】【视图切换】保存偏好后重绘现有会话，不重建 Agent；内联视图保留终端滚动输出。
/// 参数: `input` 为命令，`paths` 为配置路径，`config` 为运行配置，`runtime` 为界面
/// 返回: 是否命中视图命令；参数或保存错误通过会话提示展示。
pub(super) fn handle(
    input: &str,
    paths: &SaiPaths,
    config: &mut AppConfig,
    runtime: &mut ReplRuntime,
) -> Result<bool> {
    let Some(rest) = repl_command_rest(input, "/view") else {
        return Ok(false);
    };
    // 1. 【终端】【视图切换】验证参数与持久化成功后才修改当前显示
    let requested = render::tui_presentation::with_locale(|| {
        requested_view(
            rest,
            if runtime.fullscreen_active() {
                TuiMode::Fullscreen
            } else {
                TuiMode::Inline
            },
        )
    });
    let style = match requested {
        Ok(style) => style,
        Err(error) => {
            runtime.record_meta(error.to_string())?;
            return Ok(true);
        }
    };
    if let Err(error) = save_view(paths, style) {
        runtime.record_meta(error.to_string())?;
        return Ok(true);
    }
    // 2. 【终端】【视图切换】显式选择覆盖启动选项，历史渲染缓存按视图失效
    config.display.fullscreen = style == TuiMode::Fullscreen;
    render::tui_mode::set_override(None);
    render::configure_display_rendering(&config.display);
    if style == TuiMode::Fullscreen {
        runtime.enter_fullscreen()?;
    } else {
        runtime.leave_fullscreen()?;
    }
    runtime.redraw()?;
    let notice = render::tui_presentation::with_locale(|| {
        format!("{}: {}", t("terminal view", "终端视图"), style.as_str())
    });
    runtime.record_meta(notice)?;
    Ok(true)
}

#[cfg(test)]
#[path = "view_command_tests.rs"]
mod tests;
