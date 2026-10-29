use std::cell::Cell;

thread_local! {
    /// 当前渲染上下文是否强制展开全部折叠块
    static EXPAND_OVERRIDE: Cell<bool> = const { Cell::new(false) };
    /// 0 默认，1 强制展开当前块，2 强制折叠当前块
    static EXPAND_FORCE: Cell<u8> = const { Cell::new(0) };
}

/// 在"展开全部折叠块"的渲染上下文中执行闭包。
///
/// 备用屏 transcript 浏览等回看场景需要完整内容：思考正文、
/// 命令输出等折叠预览在此上下文内全部按展开渲染。
///
/// 参数:
/// - `render`: 渲染闭包
///
/// 返回:
/// - 闭包返回值
pub(crate) fn with_expanded_render<T>(render: impl FnOnce() -> T) -> T {
    EXPAND_OVERRIDE.with(|cell| {
        let previous = cell.replace(true);
        let result = render();
        cell.set(previous);
        result
    })
}

/// 返回当前上下文是否强制展开折叠块。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 处于展开上下文时返回 true
pub(crate) fn expand_override() -> bool {
    match EXPAND_FORCE.with(Cell::get) {
        1 => true,
        2 => false,
        _ => EXPAND_OVERRIDE.with(Cell::get),
    }
}

/// 合并单元格自身的展开标记和当前渲染上下文。
///
/// 副屏可以强制展开当前段、强制折叠其余段，而不改单元格上的标记。
///
/// 参数: `expanded` 为单元格自己的展开标记
/// 返回: 这次渲染是否展开全文
pub(crate) fn resolve_expanded(expanded: bool) -> bool {
    match EXPAND_FORCE.with(Cell::get) {
        1 => true,
        2 => false,
        _ => expanded || EXPAND_OVERRIDE.with(Cell::get),
    }
}

/// 在“只展开当前这一块”的上下文中执行闭包。
pub(crate) fn with_force_expand<T>(render: impl FnOnce() -> T) -> T {
    with_force(1, render)
}

/// 在“这一块保持折叠”的上下文中执行闭包。
pub(crate) fn with_force_collapse<T>(render: impl FnOnce() -> T) -> T {
    with_force(2, render)
}

fn with_force<T>(mode: u8, render: impl FnOnce() -> T) -> T {
    EXPAND_FORCE.with(|cell| {
        let previous = cell.replace(mode);
        let result = render();
        cell.set(previous);
        result
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_is_scoped() {
        assert!(!expand_override());
        let inside = with_expanded_render(expand_override);
        assert!(inside);
        assert!(!expand_override());
    }
}
