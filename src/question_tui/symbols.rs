/// 【终端提问】【引导标记】选项前缀显示列数，供换行与编辑光标共用。
pub(super) const OPTION_PREFIX_WIDTH: usize = 4;

/// 【终端提问】【选项标记】参数为题型与选中状态，返回单列几何标记。
pub(super) fn choice(multiple: bool, picked: bool) -> &'static str {
    match (multiple, picked) {
        (false, false) => "○",
        (false, true) => "●",
        (true, false) => "□",
        (true, true) => "■",
    }
}

/// 【终端提问】【焦点标记】参数为是否聚焦，返回箭头或等宽空白。
pub(super) fn focus(active: bool) -> &'static str {
    if active {
        "›"
    } else {
        " "
    }
}

/// 【终端提问】【进度标记】参数为是否当前题、是否已回答，返回独立的导航标记。
pub(super) fn progress(active: bool, answered: bool) -> &'static str {
    if active {
        "◆"
    } else if answered {
        "●"
    } else {
        "○"
    }
}
