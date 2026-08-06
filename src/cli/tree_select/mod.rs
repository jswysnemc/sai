mod flatten;

use super::fuzzy_select::inline_fuzzy_select;
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use crate::state::StateStore;
use anyhow::{bail, Result};
use flatten::flatten_tree;

/// 交互式选择会话树上的一个轮次。
///
/// 树按缩进与装订线展示全部分支，选中即返回目标轮次标识；
/// 调用方据此切换活动叶子。
///
/// 参数:
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 选中的轮次标识；取消时返回空
pub(super) fn select_turn_interactively(paths: &SaiPaths) -> Result<Option<String>> {
    let store = StateStore::new(paths)?;
    let tree = store.session_tree()?;
    if tree.total_turns == 0 {
        bail!(
            "{}",
            t(
                "this session has no turns yet",
                "当前会话还没有任何对话轮次",
            )
        );
    }
    let rows = flatten_tree(&tree);
    let labels = rows.iter().map(|row| row.label.clone()).collect::<Vec<_>>();
    let Some(index) = inline_fuzzy_select(&labels)? else {
        return Ok(None);
    };
    Ok(rows.get(index).map(|row| row.turn_id.clone()))
}
