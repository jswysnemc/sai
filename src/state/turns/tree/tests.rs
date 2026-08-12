use crate::state::turns::model::SESSION_ROOT_TURN_ID;
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

/// 树视图能完整表达真实分叉：一个父轮次下挂多条分支。
#[test]
fn tree_view_reports_multiple_branches_under_one_parent() {
    let temp = tempfile::tempdir().unwrap();
    let db = open_db(temp.path());

    let root = append_turn(&db, "t1", "根问题");
    for (index, id) in ["t2", "t3", "t4"].iter().enumerate() {
        db.switch_active_leaf(&root).unwrap();
        append_turn(&db, id, &format!("第 {} 种问法", index + 1));
    }

    let tree = db.session_tree().unwrap();

    assert_eq!(tree.total_turns, 4);
    assert_eq!(tree.branch_points, 1);
    assert_eq!(tree.roots.len(), 1);
    assert_eq!(tree.roots[0].children.len(), 3, "根轮次下应有三条分支");
    // 活动叶子停在最后一次提问的分支上
    assert_eq!(tree.active_leaf_id.as_deref(), Some("t4"));
    // 每条分支都能独立回溯
    let branch = db.active_branch_turns().unwrap();
    assert_eq!(branch.len(), 2);
    assert_eq!(branch[0].turn_id, "t1");
    assert_eq!(branch[1].turn_id, "t4");
}

/// 退回到中间轮次的父节点后再提问，新轮次挂在该父节点下形成分叉。
#[test]
fn resend_after_undo_branches_from_the_parent() {
    let temp = tempfile::tempdir().unwrap();
    let db = open_db(temp.path());

    let first = append_turn(&db, "t1", "第一问");
    let second = append_turn(&db, "t2", "第二问");
    let _third = append_turn(&db, "t3", "第三问");

    // 编辑第二轮：先退回它的父节点，再以新内容发起
    db.move_leaf_to_parent(&second).unwrap();
    let edited = append_turn(&db, "t4", "改写后的第二问");

    let turns = db.load_turns().unwrap();
    let edited_turn = turns.iter().find(|turn| turn.turn_id == edited).unwrap();
    assert_eq!(
        edited_turn.parent_turn_id.as_deref(),
        Some(first.as_str()),
        "改写后的轮次应当挂在被编辑轮次的父节点下"
    );
}

/// 编辑首轮时父节点为空，新轮次必须成为另一个根，而不是接到末尾。
#[test]
fn resend_after_editing_the_first_turn_creates_a_new_root() {
    let temp = tempfile::tempdir().unwrap();
    let db = open_db(temp.path());

    let first = append_turn(&db, "t1", "第一问");
    let last = append_turn(&db, "t2", "第二问");

    // 编辑首轮：退回后活动叶子为空，代表"从头开始"
    db.move_leaf_to_parent(&first).unwrap();
    let edited = append_turn(&db, "t3", "改写后的第一问");

    let turns = db.load_turns().unwrap();
    let edited_turn = turns.iter().find(|turn| turn.turn_id == edited).unwrap();
    assert_eq!(
        edited_turn.parent_turn_id.as_deref(),
        Some(SESSION_ROOT_TURN_ID),
        "改写首轮后应当挂在会话根下，而不是接到 seq 最大的 {last} 之后"
    );

    // 活动分支只剩改写后的这一轮
    let branch = db.active_branch_turns().unwrap();
    let ids: Vec<_> = branch.iter().map(|turn| turn.turn_id.clone()).collect();
    assert_eq!(ids, vec![edited]);
}

/// 编辑首轮产生的新根在重新打开数据库后必须保留。
///
/// 回归场景：线性父子回填曾在每次打开连接时执行，Web 端各请求
/// 独立开连接，编辑首轮产生的新根（parent 为 NULL）在下一次
/// timeline / 树查询时被强行接回旧分支末尾，「新建分支重发」
/// 退化成了在当前会话末尾追加。
#[test]
fn edited_first_turn_root_survives_reopening_the_database() {
    let temp = tempfile::tempdir().unwrap();
    {
        let db = open_db(temp.path());
        append_turn(&db, "t1", "第一问");
        append_turn(&db, "t2", "第二问");
        db.move_leaf_to_parent("t1").unwrap();
        append_turn(&db, "t3", "改写后的第一问");
    }

    // 模拟 Web 端的下一次请求：全新连接触发 schema 迁移路径
    let reopened = open_db(temp.path());
    let turns = reopened.load_turns().unwrap();
    let edited = turns.iter().find(|turn| turn.turn_id == "t3").unwrap();
    assert_eq!(
        edited.parent_turn_id.as_deref(),
        Some(SESSION_ROOT_TURN_ID),
        "重开连接后新根不得被线性回填接回旧分支末尾"
    );
    let tree = reopened.session_tree().unwrap();
    assert_eq!(tree.roots.len(), 2, "树上应有两个根：原首轮与改写轮");
}

/// 旧格式数据（NULL 父、空串叶子哨兵）打开时迁移到会话根哨兵。
#[test]
fn legacy_null_parents_and_empty_leaf_migrate_to_session_root() {
    let temp = tempfile::tempdir().unwrap();
    {
        let db = open_db(temp.path());
        append_turn(&db, "t1", "第一问");
        append_turn(&db, "t2", "第二问");
        // 手工降级为旧格式：首轮父置 NULL、叶子写空串旧哨兵
        let conn = db.conn.lock().unwrap();
        conn.execute(
            "UPDATE turns SET parent_turn_id = NULL WHERE turn_id = 't1'",
            [],
        )
        .unwrap();
        conn.execute(
            "UPDATE session_tree_meta SET value = '' WHERE key = 'active_leaf'",
            [],
        )
        .unwrap();
    }

    let db = open_db(temp.path());
    let turns = db.load_turns().unwrap();
    let first = turns.iter().find(|turn| turn.turn_id == "t1").unwrap();
    assert_eq!(
        first.parent_turn_id.as_deref(),
        Some(SESSION_ROOT_TURN_ID),
        "NULL 父应迁移为会话根哨兵"
    );
    // 空串叶子迁移为会话根：活动分支为空，下一轮挂根下
    assert!(db.active_branch_turns().unwrap().is_empty());
    let fresh = append_turn(&db, "t3", "新的开始");
    let turns = db.load_turns().unwrap();
    let fresh_turn = turns.iter().find(|turn| turn.turn_id == fresh).unwrap();
    assert_eq!(
        fresh_turn.parent_turn_id.as_deref(),
        Some(SESSION_ROOT_TURN_ID)
    );
}

/// 根哨兵不对外泄漏：退出首轮返回 None，树的活动叶子也为 None。
#[test]
fn session_root_sentinel_stays_internal() {
    let temp = tempfile::tempdir().unwrap();
    let db = open_db(temp.path());
    append_turn(&db, "t1", "第一问");

    let parent = db.move_leaf_to_parent("t1").unwrap();
    assert_eq!(parent, None, "退到根部对外仍表现为 None");
    assert!(db.active_branch_turns().unwrap().is_empty());
    let tree = db.session_tree().unwrap();
    assert_eq!(tree.active_leaf_id, None, "根部没有可高亮的轮次节点");

    // 会话根是合法的切换目标
    db.switch_active_leaf(SESSION_ROOT_TURN_ID).unwrap();
    assert!(db.active_branch_turns().unwrap().is_empty());
}
