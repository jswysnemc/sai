use super::TreeRow;
use crate::state::SessionTreeIndex;
use std::collections::{HashMap, HashSet};

const MAX_GUTTER_DEPTH: usize = 8;

/// 【会话分支】【终端索引】迭代生成选择器条目，深链只保留最近的缩进层级。
/// @param tree 全部轮次的扁平索引
/// @returns 保留每个轮次和会话起点的展示行
pub(super) fn flatten_index(tree: &SessionTreeIndex) -> Vec<TreeRow> {
    let active = tree.active_leaf_id.as_deref();
    let marker = if active.is_none() { "●" } else { "○" };
    let mut rows = vec![TreeRow {
        turn_id: None,
        label: format!(
            "{marker} {}",
            super::t(
                "Session start · new starting message",
                "会话起点 · 新的起始消息"
            )
        ),
    }];
    // 1. 【会话分支】【父子索引】只存节点位置，不构造递归对象
    let mut children = HashMap::<Option<&str>, Vec<usize>>::new();
    for (index, node) in tree.nodes.iter().enumerate() {
        children
            .entry(node.parent_turn_id.as_deref())
            .or_default()
            .push(index);
    }
    let roots = children.get(&None).map(Vec::as_slice).unwrap_or_default();
    let mut pending = roots
        .iter()
        .enumerate()
        .rev()
        .map(|(position, index)| (*index, Vec::<bool>::new(), position + 1 == roots.len(), 0))
        .collect::<Vec<_>>();
    let mut seen = HashSet::new();
    while let Some((index, ancestors, is_last, depth)) = pending.pop() {
        if !seen.insert(index) {
            continue;
        }
        let node = &tree.nodes[index];
        let mut gutter = if depth > MAX_GUTTER_DEPTH {
            "… ".to_string()
        } else {
            String::new()
        };
        for sibling in &ancestors {
            gutter.push_str(if *sibling { "│  " } else { "   " });
        }
        gutter.push_str(if is_last { "└─ " } else { "├─ " });
        let marker = if active == Some(node.turn_id.as_str()) {
            "●"
        } else {
            "○"
        };
        let summary = if node.user_summary.is_empty() {
            "(空)"
        } else {
            &node.user_summary
        };
        rows.push(TreeRow {
            turn_id: Some(node.turn_id.clone()),
            label: format!("{gutter}{marker} {summary}"),
        });
        // 2. 【会话分支】【深度边界】标签与暂存祖先均有界，避免长对话产生平方级缩进
        let mut next_ancestors = ancestors;
        next_ancestors.push(!is_last);
        if next_ancestors.len() > MAX_GUTTER_DEPTH {
            next_ancestors.remove(0);
        }
        if let Some(group) = children.get(&Some(node.turn_id.as_str())) {
            for (position, child) in group.iter().enumerate().rev() {
                pending.push((
                    *child,
                    next_ancestors.clone(),
                    position + 1 == group.len(),
                    depth + 1,
                ));
            }
        }
    }
    rows
}

#[cfg(test)]
mod tests;
