use super::*;
use crate::config::TuiStyle;

/// 【终端】【样式命令】解析切换参数，空参数在两种样式间切换。
/// 参数: `rest` 为命令参数，`current` 为当前样式；返回: 目标样式或用法错误。
fn requested_style(rest: &str, current: TuiStyle) -> Result<TuiStyle> {
    if rest.trim().is_empty() {
        return Ok(current.toggled());
    }
    TuiStyle::parse(rest).ok_or_else(|| {
        anyhow::anyhow!(t(
            "usage: /style [classic|deepseek]",
            "用法: /style [classic|deepseek]",
        ))
    })
}

/// 【终端】【样式保存】只修改磁盘配置中的展示偏好，避免写入 Agent 运行期覆盖。
/// 参数: `paths` 为配置路径，`style` 为目标样式；返回: 保存结果。
fn save_style(paths: &SaiPaths, style: TuiStyle) -> Result<()> {
    let mut stored = AppConfig::load_or_default(paths)?;
    stored.display.tui_style = style;
    stored.save(paths)
}

/// 【终端】【样式切换】保存偏好后重绘现有会话，不重建 Agent；经典样式恢复配置布局。
/// 参数: `input` 为命令，`paths` 为配置路径，`config` 为运行配置，`runtime` 为界面
/// 返回: 是否命中样式命令；参数或保存错误通过会话提示展示。
pub(super) fn handle(
    input: &str,
    paths: &SaiPaths,
    config: &mut AppConfig,
    runtime: &mut ReplRuntime,
) -> Result<bool> {
    let Some(rest) = repl_command_rest(input, "/style") else {
        return Ok(false);
    };
    // 1. 【终端】【样式切换】验证参数与持久化成功后才修改当前显示
    let requested = render::tui_presentation::with_locale(|| {
        requested_style(rest, render::tui_style::current())
    });
    let style = match requested {
        Ok(style) => style,
        Err(error) => {
            runtime.record_meta(error.to_string())?;
            return Ok(true);
        }
    };
    if let Err(error) = save_style(paths, style) {
        runtime.record_meta(error.to_string())?;
        return Ok(true);
    }
    // 2. 【终端】【样式切换】显式选择覆盖启动选项，历史渲染缓存按样式失效
    config.display.tui_style = style;
    render::tui_style::set_override(None);
    render::configure_display_rendering(&config.display);
    if style == TuiStyle::Deepseek {
        runtime.enter_fullscreen()?;
    } else if !config.display.fullscreen {
        runtime.leave_fullscreen()?;
    }
    runtime.redraw()?;
    let notice = render::tui_presentation::with_locale(|| {
        format!("{}: {}", t("terminal style", "终端样式"), style.as_str())
    });
    runtime.record_meta(notice)?;
    Ok(true)
}

#[cfg(test)]
#[path = "style_command_tests.rs"]
mod tests;
