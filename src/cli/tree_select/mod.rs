mod flatten;

use super::fuzzy_select::inline_fuzzy_select;
use crate::i18n::text as t;
use crate::state::StateStore;
use anyhow::Result;
use flatten::flatten_tree;

/// 会话树中可选择的位置，空白起点与取消选择分别表达。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum TreeSelection {
    Start,
    Turn(String),
}

/// 交互式选择会话树上的一个轮次。
///
/// 树按缩进与装订线展示全部分支，选中即返回目标轮次标识；
/// 调用方据此切换活动叶子。
///
/// 参数:
/// - `store`: 当前会话状态，避免选择到其他会话的树
///
/// 返回:
/// - 选中的会话起点或轮次；取消时返回空
pub(super) fn select_turn_interactively(store: &StateStore) -> Result<Option<TreeSelection>> {
    let tree = store.session_tree()?;
    let rows = flatten_tree(&tree);
    let labels = rows.iter().map(|row| row.label.clone()).collect::<Vec<_>>();
    let Some(index) = inline_fuzzy_select(&labels)? else {
        return Ok(None);
    };
    Ok(rows.get(index).map(|row| match &row.turn_id {
        Some(id) => TreeSelection::Turn(id.clone()),
        None => TreeSelection::Start,
    }))
}
