use crate::state::turns::ConversationDb;

/// 在临时目录创建对话库。
fn open_db(dir: &std::path::Path) -> ConversationDb {
    ConversationDb::open(dir).unwrap()
}

/// 追加一轮完整对话并返回其 turn_id。
fn append_turn(db: &ConversationDb, turn_id: &str, user: &str) -> String {
    db.start_turn(turn_id, user).unwrap();
    db.complete_turn(turn_id, "回答", None).unwrap();
    turn_id.to_string()
}

/// 线性追加的轮次应当形成单链，活动叶子指向最后一轮。
#[test]
fn sequential_turns_form_a_single_chain() {
    let temp = tempfile::tempdir().unwrap();
    let db = open_db(temp.path());

    let first = append_turn(&db, "t1", "第一问");
    let second = append_turn(&db, "t2", "第二问");

    let tree = db.session_tree().unwrap();
    assert_eq!(tree.roots.len(), 1);
    assert_eq!(tree.roots[0].turn_id, first);
    assert_eq!(tree.roots[0].children[0].turn_id, second);
    assert_eq!(tree.branch_points, 0);
    assert_eq!(tree.active_leaf_id.as_deref(), Some(second.as_str()));
}

/// 切回历史轮次后再提问，应当在该轮次下形成分叉而不是续在末尾。
#[test]
fn asking_after_switching_creates_a_branch() {
    let temp = tempfile::tempdir().unwrap();
    let db = open_db(temp.path());

    let first = append_turn(&db, "t1", "第一问");
    let second = append_turn(&db, "t2", "第二问");
    // 切回第一轮，相当于放弃第二轮的走向
    db.switch_active_leaf(&first).unwrap();
    let alternative = append_turn(&db, "t3", "另一种问法");

    let tree = db.session_tree().unwrap();
    assert_eq!(tree.branch_points, 1, "第一轮下应当出现分叉");
    assert_eq!(tree.roots[0].children.len(), 2);
    assert_eq!(tree.active_leaf_id.as_deref(), Some(alternative.as_str()));
    // 原分支仍然保留
    let ids: Vec<_> = tree.roots[0]
        .children
        .iter()
        .map(|child| child.turn_id.clone())
        .collect();
    assert!(ids.contains(&second));
    assert!(ids.contains(&alternative));
}

/// 活动分支只包含当前这条路径上的轮次。
#[test]
fn active_branch_excludes_other_branches() {
    let temp = tempfile::tempdir().unwrap();
    let db = open_db(temp.path());

    let first = append_turn(&db, "t1", "第一问");
    let abandoned = append_turn(&db, "t2", "被放弃的问法");
    db.switch_active_leaf(&first).unwrap();
    let kept = append_turn(&db, "t3", "保留的问法");

    let branch = db.active_branch_turns().unwrap();

    let ids: Vec<_> = branch.iter().map(|turn| turn.turn_id.clone()).collect();
    assert_eq!(ids, vec![first, kept]);
    assert!(!ids.contains(&abandoned), "另一条分支不应出现在活动历史里");
}

/// 撤销把叶子退回父轮次，轮次本身仍然保留。
#[test]
fn undo_moves_the_leaf_without_deleting_turns() {
    let temp = tempfile::tempdir().unwrap();
    let db = open_db(temp.path());

    let first = append_turn(&db, "t1", "第一问");
    let second = append_turn(&db, "t2", "第二问");

    let parent = db.move_leaf_to_parent(&second).unwrap();

    assert_eq!(parent.as_deref(), Some(first.as_str()));
    // 轮次没有被删除，树上仍然看得到
    let tree = db.session_tree().unwrap();
    assert_eq!(tree.total_turns, 2);
    assert_eq!(tree.active_leaf_id.as_deref(), Some(first.as_str()));
    // 活动历史已经回到第一轮
    let branch = db.active_branch_turns().unwrap();
    assert_eq!(branch.len(), 1);
}

/// 切换到不存在的轮次应当报错而不是静默改变状态。
#[test]
fn switching_to_a_missing_turn_fails() {
    let temp = tempfile::tempdir().unwrap();
    let db = open_db(temp.path());
    let first = append_turn(&db, "t1", "第一问");

    assert!(db.switch_active_leaf("nope").is_err());
    assert_eq!(
        db.session_tree().unwrap().active_leaf_id.as_deref(),
        Some(first.as_str())
    );
}

/// 上下文投影只包含活动分支，其它分支不得混入。
#[test]
fn conversation_context_excludes_other_branches() {
    let temp = tempfile::tempdir().unwrap();
    let db = open_db(temp.path());

    let first = append_turn(&db, "t1", "第一问");
    append_turn(&db, "t2", "被放弃的问法");
    db.switch_active_leaf(&first).unwrap();
    append_turn(&db, "t3", "保留的问法");

    let branch = db.active_branch_turns().unwrap();
    let text: String = branch
        .iter()
        .map(|turn| turn.user_content.clone())
        .collect::<Vec<_>>()
        .join("|");

    assert!(text.contains("第一问"));
    assert!(text.contains("保留的问法"));
    assert!(
        !text.contains("被放弃的问法"),
        "另一条分支不应进入上下文: {text}"
    );
}

/// 历史投影同样只取活动分支，不能按 seq 区间跨分支取轮次。
#[test]
fn history_projection_stays_on_the_active_branch() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let paths = crate::paths::SaiPaths {
        config_dir: root.join("config"),
        config_file: root.join("config/config.jsonc"),
        secrets_file: root.join("config/secrets.jsonc"),
        skills_dir: root.join("config/skills"),
        data_dir: root.join("data"),
        cache_dir: root.join("cache"),
        state_dir: root.join("state"),
        pictures_dir: root.join("pictures"),
        fish_hook_file: root.join("fish/sai.fish"),
        bash_hook_file: root.join("shell/bash-hook.sh"),
        zsh_hook_file: root.join("shell/zsh-hook.zsh"),
        powershell_hook_file: root.join("shell/powershell-hook.ps1"),
    };
    let store = crate::state::StateStore::new(&paths).unwrap();

    store.start_turn("t1", "记住 A=1").unwrap();
    store.complete_turn("t1", "ok", None).unwrap();
    store.start_turn("t2", "记住 B=2").unwrap();
    store.complete_turn("t2", "ok", None).unwrap();
    store.switch_active_leaf("t1").unwrap();

    let history = store.project_history(None).unwrap();
    let text = history
        .messages
        .iter()
        .map(|message| format!("{:?}", message))
        .collect::<Vec<_>>()
        .join("|");

    assert!(text.contains("A=1"));
    assert!(!text.contains("B=2"), "被切走的分支不应进入投影: {text}");
}
