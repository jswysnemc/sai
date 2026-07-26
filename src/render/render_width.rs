use std::cell::Cell;

thread_local! {
    /// 当前渲染上下文的目标宽度；None 表示直接查询终端
    static RENDER_WIDTH_OVERRIDE: Cell<Option<usize>> = const { Cell::new(None) };
}

/// 在指定渲染宽度上下文中执行渲染闭包。
///
/// transcript 渲染与折行必须使用同一宽度：cell 内部的表格布局、
/// 水平线与折行宽度查询在此上下文内全部返回 `width`，
/// 避免实时终端查询与外层 `wrap_block` 宽度脱节产生超宽行。
///
/// 参数:
/// - `width`: 本次渲染的目标终端列数
/// - `render`: 渲染闭包
///
/// 返回:
/// - 闭包返回值
pub(crate) fn with_render_width<T>(width: usize, render: impl FnOnce() -> T) -> T {
    RENDER_WIDTH_OVERRIDE.with(|cell| {
        let previous = cell.replace(Some(width.max(8)));
        let result = render();
        cell.set(previous);
        result
    })
}

/// 返回当前上下文的渲染宽度覆盖值。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 处于渲染上下文内时返回目标宽度
pub(crate) fn render_width_override() -> Option<usize> {
    RENDER_WIDTH_OVERRIDE.with(Cell::get)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_is_scoped_and_nested() {
        assert_eq!(render_width_override(), None);
        let outer = with_render_width(100, || {
            let inner = with_render_width(50, render_width_override);
            (render_width_override(), inner)
        });
        assert_eq!(outer, (Some(100), Some(50)));
        assert_eq!(render_width_override(), None);
    }

    #[test]
    fn override_enforces_minimum_width() {
        let width = with_render_width(2, render_width_override);
        assert_eq!(width, Some(8));
    }
}
