/// 【终端提问】【引导标记】选项前缀显示列数，供换行与编辑光标共用。
pub(super) const OPTION_PREFIX_WIDTH: usize = 4;

/// 【终端提问】【选项标记】按单选或多选题型与选中状态返回独立几何字形。
///
/// 参数:
/// - `multiple`: 是否为多选题
/// - `picked`: 当前项是否已选中
///
/// 返回:
/// - 单选题返回圆形字形，多选题返回方形复选框字形
pub(super) fn choice(multiple: bool, picked: bool) -> &'static str {
    match (multiple, picked) {
        (false, true) => "●",
        (false, false) => "○",
        (true, true) => "■",
        (true, false) => "□",
    }
}

/// 【终端提问】【焦点标记】返回当前焦点箭头或等宽空白。
///
/// 参数:
/// - `active`: 是否为当前聚焦项
///
/// 返回:
/// - 单列焦点指示字符
pub(super) fn focus(active: bool) -> &'static str {
    if active {
        "›"
    } else {
        " "
    }
}

/// 【终端提问】【进度标记】返回题签栏中当前题或已回答题目的状态字形。
///
/// 参数:
/// - `active`: 是否为当前题目
/// - `answered`: 是否已填写答案
///
/// 返回:
/// - 单列导航进度字符
pub(super) fn progress(active: bool, answered: bool) -> &'static str {
    if active {
        "◆"
    } else if answered {
        "●"
    } else {
        "○"
    }
}
