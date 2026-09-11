use super::memes_support::*;
use serde_json::json;

/// 【表情边界测试】【损坏索引】错误类型、重复记录及逃逸路径均不能被静默覆盖
/// @returns 无；源索引字节保持原样，授权外哨兵文件不变
#[tokio::test]
async fn memes_reject_corrupt_and_escaping_indexes_without_writes() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("user/sai")).unwrap();
    let protected = root.path().join("protected.png");
    std::fs::write(&protected, b"protected").unwrap();
    let runtime = runtime(root.path(), json!({}), MemeHost::new(root.path()), "");
    let normal = item("sha256:abcdef", "images/a.png", "名字");
    let mut cases = vec![
        json!([]),
        json!(null),
        json!({"memes":{}}),
        json!({"memes":[normal.clone(),normal.clone()]}),
        json!({"disabled_ids":[1]}),
    ];
    for file in [
        "../../protected.png",
        "/protected.png",
        "C:\\protected.png",
        "images/../index.json",
        "index.json",
        "images//a.png",
    ] {
        let mut escaped = normal.clone();
        escaped["file"] = json!(file);
        cases.push(json!({"memes":[escaped]}));
    }
    for case in cases {
        let bytes = serde_json::to_vec(&case).unwrap();
        std::fs::write(root.path().join("user/sai/index.json"), &bytes).unwrap();
        for (name, args) in [
            ("search_meme", json!({})),
            ("delete_meme", json!({"id":"abc","hard_delete":true})),
        ] {
            assert!(
                runtime
                    .call_tool(name, args, context(root.path()))
                    .await
                    .is_err(),
                "{case}"
            );
        }
        assert_eq!(
            std::fs::read(root.path().join("user/sai/index.json")).unwrap(),
            bytes
        );
        assert_eq!(std::fs::read(&protected).unwrap(), b"protected");
    }
    for bytes in [vec![0xff, 0xfe], vec![b' '; 1048577]] {
        std::fs::write(root.path().join("user/sai/index.json"), &bytes).unwrap();
        assert!(runtime
            .call_tool("search_meme", json!({}), context(root.path()))
            .await
            .is_err());
        assert_eq!(
            std::fs::read(root.path().join("user/sai/index.json")).unwrap(),
            bytes
        );
    }
}

/// 【表情边界测试】【独立授权】读取、写入和显示授权分别生效，伪造参数无法扩大目录
/// @returns 无；所有拒绝均不产生图片展示或新文件
#[tokio::test]
async fn memes_enforce_independent_read_write_display_and_readonly_grants() {
    let root = tempfile::tempdir().unwrap();
    seed(
        root.path(),
        "builtin",
        vec![item("sha256:abcdef", "images/base.png", "名字")],
    );
    let args = addition(root.path(), 1);
    for permission in 0..3 {
        let host = MemeHost::new(root.path());
        let runtime = configured(
            root.path(),
            json!({}),
            host.clone(),
            "",
            |grants| match permission {
                0 => grants.system.read_paths.clear(),
                1 => grants.binary.write_paths.clear(),
                _ => grants.binary.display_images = false,
            },
        );
        let (name, input) = match permission {
            0 => ("search_meme", json!({})),
            1 => ("add_meme", args.clone()),
            _ => ("show_meme", json!({"id":"abc"})),
        };
        assert!(runtime
            .call_tool(name, input, context(root.path()))
            .await
            .is_err());
        assert!(host.displays.lock().unwrap().is_empty());
    }
    let runtime = runtime(root.path(), json!({}), MemeHost::new(root.path()), "");
    let mut readonly = context(root.path());
    readonly.allow_writes = false;
    assert!(runtime.call_tool("add_meme", args, readonly).await.is_err());
    let forbidden = root.path().join("outside.png");
    std::fs::write(&forbidden, b"outside").unwrap();
    assert!(runtime
        .call_tool(
            "add_meme",
            json!({"image":forbidden,"name_zh":"名","description":"图","usage":"聊天"}),
            context(root.path())
        )
        .await
        .is_err());
    assert!(!root.path().join("user").exists());
}

/// 【表情边界测试】【排序和禁用】同分时保持用户索引顺序，覆盖及禁用按完整标识应用
/// @returns 无；候选来源、浮点分数和空数组均保持原契约
#[tokio::test]
async fn memes_search_preserves_overlay_order_and_gif_null_contract() {
    let root = tempfile::tempdir().unwrap();
    let mut first = item("sha256:aaaaaa", "images/first.gif", "用户一");
    first["animated"] = json!(true);
    seed(
        root.path(),
        "builtin",
        vec![
            item("sha256:bbbbbb", "images/base.png", "内置覆盖"),
            item("sha256:cccccc", "images/third.png", "内置三"),
        ],
    );
    seed(
        root.path(),
        "user",
        vec![first, item("sha256:bbbbbb", "images/second.png", "用户二")],
    );
    let runtime = runtime(root.path(), json!({}), MemeHost::new(root.path()), "");
    let found = call(&runtime, root.path(), "search_meme", json!({})).await;
    assert_eq!(
        found["results"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["name"]["zh"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["用户一", "用户二", "内置三"]
    );
    assert_eq!(found["results"][0]["score"], json!(2.0));
    let shown = call(&runtime, root.path(), "show_meme", json!({"id":"aaa"})).await;
    assert!(shown["animation_note"].is_string());
    assert_eq!(
        call(
            &runtime,
            root.path(),
            "search_meme",
            json!({"query":"不存在"})
        )
        .await["results"],
        json!([])
    );
}

/// 【表情边界测试】【旧整数配置】完整 u64 图片上限不能在 Lua 整数乘法中溢出
/// @returns 无；零上限拒绝非空图片，其余合法旧值仍能读取预算内的小图片
#[tokio::test]
async fn memes_image_limits_preserve_large_legacy_unsigned_values() {
    for limit in [0, 1, 1_u64 << 43, i64::MAX as u64, 1_u64 << 63, u64::MAX] {
        let root = tempfile::tempdir().unwrap();
        let runtime = runtime(
            root.path(),
            json!({"max_image_mb":limit}),
            MemeHost::new(root.path()),
            "",
        );
        let result = runtime
            .call_tool("add_meme", addition(root.path(), 1), context(root.path()))
            .await;
        if limit == 0 {
            assert!(result.is_err());
            assert!(!root.path().join("user").exists());
        } else {
            assert!(result.is_ok(), "limit={limit}: {result:?}");
        }
    }
}

/// 【表情边界测试】【图片链接】索引不能通过符号链接展示所属库 images 目录以外的文件
/// @returns 无；即使目标本身位于其他已授权读取目录，显示仍失败
#[cfg(unix)]
#[tokio::test]
async fn memes_display_rejects_symlinks_outside_library_images() {
    let root = tempfile::tempdir().unwrap();
    seed(
        root.path(),
        "builtin",
        vec![item("sha256:abcdef", "images/base.png", "图片")],
    );
    let outside = addition(root.path(), 1)["image"]
        .as_str()
        .unwrap()
        .to_string();
    let linked = root.path().join("builtin/sai/images/base.png");
    std::fs::remove_file(&linked).unwrap();
    std::os::unix::fs::symlink(&outside, &linked).unwrap();
    let host = MemeHost::new(root.path());
    let runtime = runtime(root.path(), json!({}), host.clone(), "");
    let result = runtime
        .call_tool("show_meme", json!({"id":"abc"}), context(root.path()))
        .await;
    assert!(result.is_err());
    assert!(host.displays.lock().unwrap().is_empty());
    assert_eq!(std::fs::read(outside).unwrap(), b"image 1");
}
