use std::cell::Cell;

thread_local! {
    /// 当前渲染上下文是否强制展开全部折叠块
    static EXPAND_OVERRIDE: Cell<bool> = const { Cell::new(false) };
    /// 0 默认，1 强制展开当前块，2 强制折叠当前块
    static EXPAND_FORCE: Cell<u8> = const { Cell::new(0) };
    /// 全屏分段渲染时命令与输出各自的展开状态：(命令, 输出)，None 表示不分段
    static PART_FORCE: Cell<Option<(bool, bool)>> = const { Cell::new(None) };
    /// 当前正在折叠的是命令卡片的哪一段
    static CURRENT_PART: Cell<Option<ExpandPart>> = const { Cell::new(None) };
}

/// 命令卡片中可以独立展开的一段。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) enum ExpandPart {
    /// `$ command` 命令行
    Command,
    /// 命令的标准输出与错误输出
    Output,
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
    // 分段上下文中命令与输出各自决定，不受整块强制状态影响
    if let (Some(part), Some((command, output))) =
        (CURRENT_PART.with(Cell::get), PART_FORCE.with(Cell::get))
    {
        return match part {
            ExpandPart::Command => command,
            ExpandPart::Output => output,
        };
    }
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

/// 【折叠渲染】【分段展开】在命令与输出分别指定展开状态的上下文中执行闭包。
///
/// 全屏视图点击命令行只展开命令，点击输出只展开输出；
/// 命令卡片之外的折叠块不受影响，仍按外层上下文决定。
///
/// 参数:
/// - `command`: 命令行是否展开
/// - `output`: 输出是否展开
/// - `render`: 渲染闭包
///
/// 返回:
/// - 闭包返回值
pub(crate) fn with_part_expansion<T>(command: bool, output: bool, render: impl FnOnce() -> T) -> T {
    PART_FORCE.with(|cell| {
        let previous = cell.replace(Some((command, output)));
        let result = render();
        cell.set(previous);
        result
    })
}

/// 【折叠渲染】【当前分段】标记闭包内的折叠属于命令卡片的哪一段。
///
/// 参数:
/// - `part`: 命令或输出
/// - `render`: 折叠闭包
///
/// 返回:
/// - 闭包返回值
pub(crate) fn within_part<T>(part: ExpandPart, render: impl FnOnce() -> T) -> T {
    CURRENT_PART.with(|cell| {
        let previous = cell.replace(Some(part));
        let result = render();
        cell.set(previous);
        result
    })
}

/// 命令段与输出段之间的分界标记，独占一行，全屏视图切分后即删除。
///
/// 取 Unicode 私用区字符，正常输出中不会出现。
pub(crate) const PART_BOUNDARY: &str = "\u{F8FF}";

/// 【折叠渲染】【分界标记】分段渲染时返回带换行的分界行，其余场景为空串。
///
/// 返回:
/// - 分界行文本
pub(crate) fn part_boundary() -> String {
    if part_expansion_active() {
        format!("\n{PART_BOUNDARY}")
    } else {
        String::new()
    }
}

/// 判断当前是否处于分段展开上下文。
///
/// 返回:
/// - 全屏分段渲染时为 true
pub(crate) fn part_expansion_active() -> bool {
    PART_FORCE.with(Cell::get).is_some()
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

    /// 分段上下文中命令与输出各自生效，段外仍按整块强制状态。
    #[test]
    fn part_expansion_is_resolved_per_part() {
        with_force_collapse(|| {
            with_part_expansion(true, false, || {
                assert!(within_part(ExpandPart::Command, || resolve_expanded(false)));
                assert!(!within_part(ExpandPart::Output, || resolve_expanded(true)));
                assert!(!resolve_expanded(true), "段外仍受整块折叠约束");
                assert!(part_expansion_active());
            });
        });
        assert!(!part_expansion_active());
        assert!(within_part(ExpandPart::Output, || resolve_expanded(true)));
    }

    #[test]
    fn override_is_scoped() {
        assert!(!expand_override());
        let inside = with_expanded_render(expand_override);
        assert!(inside);
        assert!(!expand_override());
    }
}
