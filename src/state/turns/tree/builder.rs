use super::model::{summarize, SessionTree, TurnTreeNode};
use crate::state::turns::model::Turn;
use std::collections::{BTreeMap, HashSet};

/// 把扁平轮次列表还原成会话树。
///
/// 按 `parent_turn_id` 建立父子关系；父轮次缺失的节点提升为根，
/// 保证损坏数据也能完整展示而不是凭空消失。
///
/// 参数:
/// - `turns`: 会话内全部轮次
/// - `active_leaf_id`: 当前活动叶子轮次
///
/// 返回:
/// - 会话树视图
pub(super) fn build_tree(turns: &[Turn], active_leaf_id: Option<String>) -> SessionTree {
    // 1. 先建节点表，同时记录哪些 turn_id 真实存在
    let known: HashSet<&str> = turns.iter().map(|turn| turn.turn_id.as_str()).collect();
    let mut nodes: BTreeMap<i64, TurnTreeNode> = BTreeMap::new();
    for turn in turns {
        // 父轮次不存在时按根处理，避免整棵子树被丢弃
        let parent = turn
            .parent_turn_id
            .as_deref()
            .filter(|parent| known.contains(parent))
            .map(str::to_string);
        nodes.insert(
            turn.seq,
            TurnTreeNode {
                turn_id: turn.turn_id.clone(),
                parent_turn_id: parent,
                seq: turn.seq,
                user_summary: summarize(&turn.user_content),
                assistant_summary: summarize(&turn.assistant_content),
                status: turn.status.as_display_str().to_string(),
                timestamp: turn.user_timestamp.clone(),
                children: Vec::new(),
            },
        );
    }

    // 2. 按 seq 升序自底向上挂载：子节点先成形，再整体移交给父节点
    let mut by_id: BTreeMap<String, TurnTreeNode> = BTreeMap::new();
    let mut roots = Vec::new();
    let mut children_of: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for node in nodes.values() {
        match node.parent_turn_id.as_deref() {
            Some(parent) => children_of
                .entry(parent.to_string())
                .or_default()
                .push(node.turn_id.clone()),
            None => roots.push(node.turn_id.clone()),
        }
        by_id.insert(node.turn_id.clone(), node.clone());
    }

    // 3. 统计分叉点：拥有多个直接子节点的轮次
    let branch_points = children_of
        .values()
        .filter(|children| children.len() > 1)
        .count();

    let roots = roots
        .into_iter()
        .map(|root| attach_children(root, &by_id, &children_of))
        .collect();

    SessionTree {
        roots,
        active_leaf_id,
        total_turns: turns.len(),
        branch_points,
    }
}

/// 递归挂载子节点。
///
/// 参数:
/// - `turn_id`: 当前节点标识
/// - `by_id`: 节点表
/// - `children_of`: 父节点到子节点列表的映射
///
/// 返回:
/// - 已挂载全部后代的节点
fn attach_children(
    turn_id: String,
    by_id: &BTreeMap<String, TurnTreeNode>,
    children_of: &BTreeMap<String, Vec<String>>,
) -> TurnTreeNode {
    let mut node = by_id
        .get(&turn_id)
        .cloned()
        .unwrap_or_else(|| unreachable_node(&turn_id));
    if let Some(children) = children_of.get(&turn_id) {
        node.children = children
            .iter()
            .map(|child| attach_children(child.clone(), by_id, children_of))
            .collect();
        node.children.sort_by_key(|child| child.seq);
    }
    node
}

/// 构造占位节点，仅在节点表缺失时使用。
///
/// 参数:
/// - `turn_id`: 轮次标识
///
/// 返回:
/// - 占位节点
fn unreachable_node(turn_id: &str) -> TurnTreeNode {
    TurnTreeNode {
        turn_id: turn_id.to_string(),
        parent_turn_id: None,
        seq: 0,
        user_summary: String::new(),
        assistant_summary: String::new(),
        status: "unknown".to_string(),
        timestamp: String::new(),
        children: Vec::new(),
    }
}

/// 收集从根到指定轮次的完整路径。
///
/// 切换分支后需要按这条路径重放历史，因此顺序必须是根在前、目标在后。
///
/// 参数:
/// - `turns`: 会话内全部轮次
/// - `leaf_id`: 目标叶子轮次
///
/// 返回:
/// - 根到目标的轮次标识；目标不存在时返回空
pub(super) fn path_to_leaf(turns: &[Turn], leaf_id: &str) -> Vec<String> {
    let parents: BTreeMap<&str, Option<&str>> = turns
        .iter()
        .map(|turn| (turn.turn_id.as_str(), turn.parent_turn_id.as_deref()))
        .collect();
    if !parents.contains_key(leaf_id) {
        return Vec::new();
    }
    let mut path = Vec::new();
    let mut seen = HashSet::new();
    let mut cursor = Some(leaf_id);
    // 自叶向根回溯；seen 兜住数据损坏导致的环，避免死循环
    while let Some(current) = cursor {
        if !seen.insert(current) {
            break;
        }
        path.push(current.to_string());
        cursor = parents.get(current).copied().flatten();
    }
    path.reverse();
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::turns::model::TurnStatus;

    /// 构造测试轮次。
    fn turn(id: &str, seq: i64, parent: Option<&str>) -> Turn {
        Turn {
            turn_id: id.to_string(),
            seq,
            user_content: format!("问题 {id}"),
            user_image_urls: Vec::new(),
            user_timestamp: String::new(),
            assistant_content: format!("回答 {id}"),
            assistant_reasoning: None,
            assistant_timestamp: None,
            status: TurnStatus::Completed,
            tool_reports: Vec::new(),
            duration_ms: 0,
            parent_turn_id: parent.map(str::to_string),
            model: None,
            error: None,
        }
    }

    /// 线性历史构成单链，没有分叉点。
    #[test]
    fn builds_a_linear_chain() {
        let turns = vec![
            turn("a", 1, None),
            turn("b", 2, Some("a")),
            turn("c", 3, Some("b")),
        ];

        let tree = build_tree(&turns, Some("c".to_string()));

        assert_eq!(tree.roots.len(), 1);
        assert_eq!(tree.branch_points, 0);
        assert_eq!(tree.total_turns, 3);
        assert_eq!(tree.roots[0].children[0].children[0].turn_id, "c");
    }

    /// 同一父轮次下的多个子轮次构成分叉。
    #[test]
    fn detects_branch_points() {
        let turns = vec![
            turn("a", 1, None),
            turn("b", 2, Some("a")),
            turn("c", 3, Some("a")),
        ];

        let tree = build_tree(&turns, Some("c".to_string()));

        assert_eq!(tree.branch_points, 1);
        assert_eq!(tree.roots[0].children.len(), 2);
        // 同级按 seq 升序
        assert_eq!(tree.roots[0].children[0].turn_id, "b");
        assert_eq!(tree.roots[0].children[1].turn_id, "c");
    }

    /// 父轮次缺失时节点提升为根，不会丢失。
    #[test]
    fn promotes_orphans_to_roots() {
        let turns = vec![turn("a", 1, None), turn("orphan", 2, Some("missing"))];

        let tree = build_tree(&turns, None);

        assert_eq!(tree.roots.len(), 2);
        assert_eq!(tree.total_turns, 2);
    }

    /// 路径从根到叶按顺序返回。
    #[test]
    fn resolves_path_from_root_to_leaf() {
        let turns = vec![
            turn("a", 1, None),
            turn("b", 2, Some("a")),
            turn("c", 3, Some("b")),
        ];

        assert_eq!(path_to_leaf(&turns, "c"), vec!["a", "b", "c"]);
        assert_eq!(path_to_leaf(&turns, "a"), vec!["a"]);
    }

    /// 分支路径只含自己那一条链。
    #[test]
    fn branch_path_excludes_sibling_turns() {
        let turns = vec![
            turn("a", 1, None),
            turn("b", 2, Some("a")),
            turn("c", 3, Some("a")),
        ];

        assert_eq!(path_to_leaf(&turns, "c"), vec!["a", "c"]);
    }

    /// 不存在的叶子返回空路径。
    #[test]
    fn missing_leaf_has_no_path() {
        let turns = vec![turn("a", 1, None)];

        assert!(path_to_leaf(&turns, "nope").is_empty());
    }

    /// 数据损坏形成环时不死循环。
    #[test]
    fn survives_cyclic_parent_links() {
        let turns = vec![turn("a", 1, Some("b")), turn("b", 2, Some("a"))];

        let path = path_to_leaf(&turns, "a");

        assert!(path.len() <= 2);
    }
}
