use super::*;

/// 【目标】【切换回归】主轮次跟随目标替换并重置用量水位，旧子任务不能给新目标计费。
/// 参数：无；返回通过断言。
#[test]
fn replacing_goal_rebinds_primary_turn_without_old_usage() {
    let root = tempfile::tempdir().unwrap();
    let store = GoalStore::new(root.path().join("goal.json"));
    let first = store.replace("first", None, false).unwrap();
    let primary =
        GoalAccounting::new(store.clone(), "turn:one".into(), Some(first.id.clone())).unwrap();
    let child = GoalAccounting::new(store.clone(), "subagent:one".into(), Some(first.id)).unwrap();
    let usage = Usage {
        total_tokens: 30,
        ..Default::default()
    };
    primary.record(Some(&usage)).unwrap();
    let next = store.replace("second", Some(50), true).unwrap();
    assert_eq!(primary.current_goal().unwrap().unwrap().id, next.id);
    assert!(child.current_goal().unwrap().is_none());
    child.record(Some(&usage)).unwrap();
    primary.record(Some(&usage)).unwrap();
    assert_eq!(store.get().unwrap().unwrap().tokens_used, 30);
    primary.record(Some(&usage)).unwrap();
    assert_eq!(
        primary.current_goal().unwrap().unwrap().status,
        super::super::GoalStatus::BudgetLimited
    );
}
