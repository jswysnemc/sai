use std::cell::Cell;

thread_local! {
    /// 当前渲染上下文是否强制展开全部折叠块
    static EXPAND_OVERRIDE: Cell<bool> = const { Cell::new(false) };
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
    EXPAND_OVERRIDE.with(Cell::get)
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
