use super::*;
use std::time::{Duration, Instant};

/// 【终端】【技能匹配】按字符顺序匹配非连续缩写，并保持大小写不敏感
/// 参数: 无；返回: 无，缩写无法命中技能名称时断言失败
#[test]
fn skill_fuzzy_matching_accepts_non_contiguous_names() {
    let skills = vec![("git-checkout".into(), String::new())];
    let mut completion = MentionCompletion::default();
    for input in ["#gco", "#GCO"] {
        let items = completion.query(input, input.chars().count(), &skills);
        assert_eq!(items.len(), 1, "{input} 应匹配 git-checkout");
        assert_eq!(items[0].insert, "#git-checkout");
    }
    assert!(completion.query("#ocg", 4, &skills).is_empty());
}

/// 【终端】【技能匹配】名称完全匹配优先，其次名称匹配，最后是仅描述匹配
/// 参数: 无；返回: 无，弱相关结果排在名称匹配前面时断言失败
#[test]
fn skill_fuzzy_matching_ranks_name_above_description() {
    let skills = vec![
        ("helper".into(), "git".into()),
        ("git-checkout".into(), String::new()),
        ("git".into(), String::new()),
    ];
    let items = MentionCompletion::default().query("#git", 4, &skills);
    let names = items
        .iter()
        .map(|item| item.insert.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, ["#git", "#git-checkout", "#helper"]);
}

/// 【终端】【技能匹配】同为名称模糊匹配时，紧密连续命中优先于分散命中
/// 参数: 无；返回: 无，候选没有按相关度排序时断言失败
#[test]
fn skill_fuzzy_matching_ranks_compact_matches_first() {
    let skills = vec![
        ("g-long-c-long-o".into(), String::new()),
        ("gco-helper".into(), String::new()),
    ];
    let items = MentionCompletion::default().query("#gco", 4, &skills);
    let names = items
        .iter()
        .map(|item| item.insert.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, ["#gco-helper", "#g-long-c-long-o"]);
}

/// 【终端】【技能匹配】描述支持非连续匹配和中文，空查询及同分结果顺序保持稳定
/// 参数: 无；返回: 无，描述无法检索或结果顺序漂移时断言失败
#[test]
fn skill_fuzzy_matching_supports_descriptions_and_stable_ties() {
    let skills = vec![
        ("z-helper".into(), "web search 网页搜索".into()),
        ("a-helper".into(), "web search 网页搜索".into()),
    ];
    let mut completion = MentionCompletion::default();
    for input in ["#", "#ws", "#网页索", "#WS"] {
        let items = completion.query(input, input.chars().count(), &skills);
        let names = items
            .iter()
            .map(|item| item.insert.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, ["#z-helper", "#a-helper"], "{input}");
    }
}

/// 【终端】【技能补全】空查询和关键词查询均保留全部匹配技能
/// 参数: 无；返回: 无，候选遭到截断时断言失败
#[test]
fn skill_completion_keeps_all_matches() {
    let skills = (0..12)
        .map(|index| (format!("skill-{index:02}"), "shared description".into()))
        .collect::<Vec<_>>();
    let mut completion = MentionCompletion::default();
    for input in ["#", "#skill", "#SHARED"] {
        let items = completion.query(input, input.chars().count(), &skills);
        assert_eq!(items.len(), skills.len(), "查询 {input} 丢失匹配技能");
        assert_eq!(items.last().unwrap().insert, "#skill-11");
    }
    assert!(completion.query("#missing", 8, &skills).is_empty());
}

/// 请求序号、光标或输入任何一项变化时拒绝旧结果
#[test]
fn stale_responses_cannot_replace_current_candidates() {
    let snapshot = Snapshot {
        input: "@src".into(),
        cursor: 4,
        cwd: PathBuf::from("/workspace"),
    };
    let mut completion = MentionCompletion {
        id: 7,
        snapshot: Some(snapshot.clone()),
        pending: true,
        ..Default::default()
    };
    for (id, input, cursor) in [(6, "@src", 4), (7, "read @src", 9), (7, "@src", 3)] {
        assert!(!completion.accept(Response {
            id,
            snapshot: Snapshot {
                input: input.into(),
                cursor,
                cwd: snapshot.cwd.clone()
            },
            items: vec![],
        }));
        assert!(completion.pending());
    }
    assert!(completion.accept(Response {
        id: 7,
        snapshot,
        items: vec![]
    }));
    assert!(!completion.pending());
}

/// 目录查询不阻塞调用者，快速改词只返回最后一次输入对应的候选
#[test]
fn background_scan_returns_latest_query_and_cancels_on_exit() {
    // 1. 【终端】【补全回归】目录名固定包含查询词，避免随机临时路径掩盖过滤错误
    let root = tempfile::Builder::new()
        .prefix("sai-completion-b-")
        .tempdir()
        .unwrap();
    std::fs::write(root.path().join("alpha.txt"), "").unwrap();
    std::fs::write(root.path().join("beta.txt"), "").unwrap();
    let first = format!("@{}/a", root.path().display());
    let last = format!("@{}/b", root.path().display());
    let mut completion = MentionCompletion::default();
    completion.query(&first, first.chars().count(), &[]);
    let mut items = completion.query(&last, last.chars().count(), &[]);
    let deadline = Instant::now() + Duration::from_secs(3);
    while completion.pending() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
        items = completion.query(&last, last.chars().count(), &[]);
    }
    assert!(!completion.pending());
    assert_eq!(items.len(), 1);
    assert!(items[0].insert.ends_with("beta.txt"));
    completion.query(&first, first.chars().count(), &[]);
    assert!(completion.query("plain input", 11, &[]).is_empty());
    assert!(!completion.pending());
}

/// 【终端】【补全回归】相对与绝对目录前缀不参与过滤，显示及插入路径仍保持完整
/// 参数: 无；返回无，目录名导致无关文件命中时断言失败
#[test]
fn directory_prefix_cannot_match_file_filter() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("beta-dir");
    std::fs::create_dir(&directory).unwrap();
    for name in ["alpha.txt", "beta.txt"] {
        std::fs::write(directory.join(name), "").unwrap();
    }
    for prefix in ["beta-dir/".to_string(), format!("{}/", directory.display())] {
        let mut cache = None;
        for filter in ["beta", "BE"] {
            let items = super::super::files::complete(
                root.path(),
                &format!("{prefix}{filter}"),
                &mut cache,
                || false,
            )
            .unwrap();
            assert_eq!(items.len(), 1, "{items:?}");
            assert_eq!(items[0].label, format!("{prefix}beta.txt"));
            assert_eq!(items[0].insert, format!("@{prefix}beta.txt"));
        }
    }
}

/// 取消中的目录扫描不应缓存部分条目，恢复查询后仍能找到完整列表
#[test]
fn interrupted_scan_does_not_poison_directory_cache() {
    let root = tempfile::tempdir().unwrap();
    for index in 0..30 {
        std::fs::write(root.path().join(format!("file-{index:02}")), "").unwrap();
    }
    let mut cache = None;
    let checks = std::cell::Cell::new(0);
    let result = super::super::files::complete(root.path(), "", &mut cache, || {
        checks.set(checks.get() + 1);
        checks.get() > 3
    });
    assert!(result.is_none());
    assert!(cache.is_none());
    let result =
        super::super::files::complete(root.path(), "file-29", &mut cache, || false).unwrap();
    assert_eq!(result[0].label, "file-29");
}
