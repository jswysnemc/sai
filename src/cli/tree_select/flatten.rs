use crate::state::{SessionTree, TurnTreeNode};

/// 压平后的树行，供选择器展示与回选。
#[derive(Debug, Clone)]
pub(crate) struct TreeRow {
    /// 该行对应的轮次
    pub(crate) turn_id: String,
    /// 已经拼好装订线与摘要的展示文本
    pub(crate) label: String,
    /// 是否为当前活动叶子
    pub(crate) is_active: bool,
}

/// 把会话树压平成带装订线的行列表。
///
/// 树在终端里只能逐行呈现，父子关系靠 `├─`、`└─` 与祖先竖线表达。
/// 同级按 seq 升序，保证与对话发生顺序一致。
///
/// 参数:
/// - `tree`: 会话树
///
/// 返回:
/// - 自上而下的展示行
pub(crate) fn flatten_tree(tree: &SessionTree) -> Vec<TreeRow> {
    let mut rows = Vec::new();
    let active = tree.active_leaf_id.as_deref();
    let root_count = tree.roots.len();
    for (index, root) in tree.roots.iter().enumerate() {
        // 多个根时根之间也需要装订线，单根则从零缩进直接展开
        let ancestors = if root_count > 1 {
            vec![index + 1 < root_count]
        } else {
            Vec::new()
        };
        push_node(
            root,
            &ancestors,
            root_count > 1,
            index + 1 == root_count,
            active,
            &mut rows,
        );
    }
    rows
}

/// 递归写入单个节点及其后代。
///
/// 参数:
/// - `node`: 当前节点
/// - `ancestors`: 各祖先层级是否还有后续兄弟，决定是否画竖线
/// - `has_connector`: 当前层级是否需要画连接符
/// - `is_last`: 是否为同级最后一个
/// - `active`: 活动叶子标识
/// - `rows`: 输出行
///
/// 返回:
/// - 无
fn push_node(
    node: &TurnTreeNode,
    ancestors: &[bool],
    has_connector: bool,
    is_last: bool,
    active: Option<&str>,
    rows: &mut Vec<TreeRow>,
) {
    let is_active = active == Some(node.turn_id.as_str());
    rows.push(TreeRow {
        turn_id: node.turn_id.clone(),
        label: render_label(node, ancestors, has_connector, is_last, is_active),
        is_active,
    });
    // 1. 子层的祖先竖线：当前层若还有后续兄弟就要延续竖线
    let mut child_ancestors = ancestors.to_vec();
    if has_connector {
        child_ancestors.push(!is_last);
    }
    let count = node.children.len();
    for (index, child) in node.children.iter().enumerate() {
        push_node(
            child,
            &child_ancestors,
            true,
            index + 1 == count,
            active,
            rows,
        );
    }
}

/// 渲染单行文本：装订线 + 活动标记 + 摘要。
///
/// 参数:
/// - `node`: 当前节点
/// - `ancestors`: 各祖先层级是否还有后续兄弟
/// - `has_connector`: 是否需要连接符
/// - `is_last`: 是否为同级最后一个
/// - `is_active`: 是否为活动叶子
///
/// 返回:
/// - 展示文本
fn render_label(
    node: &TurnTreeNode,
    ancestors: &[bool],
    has_connector: bool,
    is_last: bool,
    is_active: bool,
) -> String {
    let mut gutter = String::new();
    for has_sibling in ancestors {
        gutter.push_str(if *has_sibling { "│  " } else { "   " });
    }
    if has_connector {
        gutter.push_str(if is_last { "└─ " } else { "├─ " });
    }
    // 活动叶子用实心圆点标记，其余用空心
    let marker = if is_active { "●" } else { "○" };
    let summary = if node.user_summary.is_empty() {
        "(空)".to_string()
    } else {
        node.user_summary.clone()
    };
    format!("{gutter}{marker} {summary}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造测试节点。
    fn node(id: &str, seq: i64, children: Vec<TurnTreeNode>) -> TurnTreeNode {
        TurnTreeNode {
            turn_id: id.to_string(),
            parent_turn_id: None,
            seq,
            user_summary: format!("问题{id}"),
            assistant_summary: String::new(),
            status: "completed".to_string(),
            timestamp: String::new(),
            children,
        }
    }

    /// 单链不出现分叉连接符。
    #[test]
    fn linear_chain_uses_simple_gutters() {
        let tree = SessionTree {
            roots: vec![node("a", 1, vec![node("b", 2, vec![])])],
            active_leaf_id: Some("b".to_string()),
            total_turns: 2,
            branch_points: 0,
        };

        let rows = flatten_tree(&tree);

        assert_eq!(rows.len(), 2);
        assert!(rows[0].label.starts_with("○ "), "{}", rows[0].label);
        assert!(rows[1].label.contains("└─"), "{}", rows[1].label);
        assert!(rows[1].is_active);
    }

    /// 分叉处前一个用 ├─，最后一个用 └─。
    #[test]
    fn branches_use_tee_and_corner_connectors() {
        let tree = SessionTree {
            roots: vec![node("a", 1, vec![node("b", 2, vec![]), node("c", 3, vec![])])],
            active_leaf_id: Some("c".to_string()),
            total_turns: 3,
            branch_points: 1,
        };

        let rows = flatten_tree(&tree);

        assert_eq!(rows.len(), 3);
        assert!(rows[1].label.contains("├─"), "{}", rows[1].label);
        assert!(rows[2].label.contains("└─"), "{}", rows[2].label);
    }

    /// 非最后分支的子节点保留祖先竖线。
    #[test]
    fn keeps_ancestor_bars_for_deep_nodes() {
        let tree = SessionTree {
            roots: vec![node(
                "a",
                1,
                vec![node("b", 2, vec![node("d", 4, vec![])]), node("c", 3, vec![])],
            )],
            active_leaf_id: None,
            total_turns: 4,
            branch_points: 1,
        };

        let rows = flatten_tree(&tree);

        // b 不是最后一个兄弟，因此 d 这一行要先画竖线再画连接符
        let deep = rows.iter().find(|row| row.turn_id == "d").unwrap();
        assert!(deep.label.contains("│"), "{}", deep.label);
    }

    /// 活动叶子用实心标记区分。
    #[test]
    fn marks_the_active_leaf() {
        let tree = SessionTree {
            roots: vec![node("a", 1, vec![])],
            active_leaf_id: Some("a".to_string()),
            total_turns: 1,
            branch_points: 0,
        };

        let rows = flatten_tree(&tree);

        assert!(rows[0].label.contains('●'));
        assert!(rows[0].is_active);
    }
}
