/// 【终端提问】【引导标记】选项前缀显示列数，供换行与编辑光标共用。
pub(super) const OPTION_PREFIX_WIDTH: usize = 4;

/// 【终端提问】【选项标记】按选中状态返回统一单列圆形字形。
///
/// 参数:
/// - `_multiple`: 是否为多选题
/// - `picked`: 当前项是否已选中
///
/// 返回:
/// - 已选中返回实心圆，未选中返回空心圆
pub(super) fn choice(_multiple: bool, picked: bool) -> &'static str {
    if picked {
        "●"
    } else {
        "○"
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
