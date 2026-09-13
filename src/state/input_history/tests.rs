use super::*;

/// 【输入历史】【并发测试】不同终端同时保存时，所有完整输入都必须保留。
/// 参数: 无
/// 返回: 无，任何记录被其他写入覆盖时断言失败
#[test]
fn simultaneous_writers_preserve_all_entries() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
    std::thread::scope(|scope| {
        for index in 0..8 {
            let barrier = barrier.clone();
            let paths = &paths;
            scope.spawn(move || {
                barrier.wait();
                append_input_history(paths, &format!("parallel-entry-{index}")).unwrap();
            });
        }
    });
    assert_eq!(load_input_history_entries(&paths).unwrap().len(), 8);
}

/// 【输入历史】【快照测试】相同标签但内容不同的原子块必须分别保存。
/// 参数: 无
/// 返回: 无，后一条输入覆盖前一条附件时断言失败
#[test]
fn identical_markers_with_different_payloads_remain_distinct() {
    let temp = tempfile::tempdir().unwrap();
    let paths = SaiPaths::for_tests(temp.path());
    for content in ["first payload", "second payload"] {
        append_input_history_entry(
            &paths,
            &InputHistoryEntry {
                text: "[text 1 500 chars]".into(),
                attachments: vec![InputHistoryAttachment {
                    marker: "[text 1 500 chars]".into(),
                    content: content.into(),
                    kind: InputHistoryAttachmentKind::Text,
                }],
            },
        )
        .unwrap();
    }
    append_input_history(&paths, "legacy plain text").unwrap();
    let entries = load_input_history_entries(&paths).unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].attachments[0].content, "first payload");
    assert_eq!(entries[1].attachments[0].content, "second payload");
    assert!(entries[2].attachments.is_empty());
}

/// 构造指向临时目录的路径配置。
///
/// 参数:
/// - `root`: 临时根目录
///
/// 返回:
/// - 测试用路径配置
fn test_paths(root: PathBuf) -> SaiPaths {
    SaiPaths {
        config_dir: root.join("config"),
        config_file: root.join("config/config.jsonc"),
        secrets_file: root.join("config/secrets.json"),
        skills_dir: root.join("skills"),
        data_dir: root.join("data"),
        cache_dir: root.join("cache"),
        state_dir: root.join("state"),
        pictures_dir: root.join("pictures"),
        fish_hook_file: root.join("shell/sai.fish"),
        bash_hook_file: root.join("shell/bash-hook.sh"),
        zsh_hook_file: root.join("shell/zsh-hook.zsh"),
        powershell_hook_file: root.join("shell/powershell-hook.ps1"),
    }
}

/// 验证历史按时间正序累积且跨"会话"共享（同一路径重复读取）。
#[test]
fn appends_entries_in_order() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path().to_path_buf());
    append_input_history(&paths, "first").unwrap();
    append_input_history(&paths, "second").unwrap();
    assert_eq!(load_input_history(&paths).unwrap(), vec!["first", "second"]);
}

/// 验证连续重复输入不产生新条目。
#[test]
fn skips_consecutive_duplicates() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path().to_path_buf());
    append_input_history(&paths, "same").unwrap();
    append_input_history(&paths, "same").unwrap();
    assert_eq!(load_input_history(&paths).unwrap(), vec!["same"]);
}

/// 验证重复出现的旧输入被移动到末尾而不是留下两份。
#[test]
fn moves_repeated_entry_to_end() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path().to_path_buf());
    append_input_history(&paths, "a").unwrap();
    append_input_history(&paths, "b").unwrap();
    append_input_history(&paths, "a").unwrap();
    assert_eq!(load_input_history(&paths).unwrap(), vec!["b", "a"]);
}

/// 验证超出上限时丢弃最旧条目。
#[test]
fn trims_to_limit() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path().to_path_buf());
    for index in 0..(INPUT_HISTORY_LIMIT + 10) {
        append_input_history(&paths, &format!("entry-{index}")).unwrap();
    }
    let entries = load_input_history(&paths).unwrap();
    assert_eq!(entries.len(), INPUT_HISTORY_LIMIT);
    assert_eq!(entries.first().unwrap(), "entry-10");
    assert_eq!(
        entries.last().unwrap(),
        &format!("entry-{}", INPUT_HISTORY_LIMIT + 9)
    );
}

/// 验证空白输入不进入历史。
#[test]
fn ignores_blank_entries() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path().to_path_buf());
    append_input_history(&paths, "   \n  ").unwrap();
    assert!(load_input_history(&paths).unwrap().is_empty());
}

/// 验证损坏行被跳过而不是导致读取失败。
#[test]
fn skips_corrupt_lines() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path().to_path_buf());
    append_input_history(&paths, "good").unwrap();
    let path = history_file(&paths);
    let mut content = fs::read_to_string(&path).unwrap();
    content.push_str("{not json}\n");
    fs::write(&path, content).unwrap();
    assert_eq!(load_input_history(&paths).unwrap(), vec!["good"]);
}
