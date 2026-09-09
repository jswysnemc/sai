use super::*;
use flate2::{write::GzEncoder, Compression};

/// 【归档测试】【字节样本】直接写入 tar 头以覆盖常规打包器会预先拒绝的恶意路径。
/// @param entries 路径、类型与正文样本
/// @returns 固定 gzip 压缩归档
fn fixture(entries: &[(&str, u8, &[u8])]) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    for (path, kind, bytes) in entries {
        let mut header = tar::Header::new_gnu();
        header.as_mut_bytes()[..path.len()].copy_from_slice(path.as_bytes());
        header.set_entry_type(tar::EntryType::new(*kind));
        header.set_mode(0o755);
        header.set_size(bytes.len() as u64);
        header.set_cksum();
        builder.append(&header, *bytes).unwrap();
    }
    let mut gzip = GzEncoder::new(Vec::new(), Compression::default());
    gzip.write_all(&builder.into_inner().unwrap()).unwrap();
    gzip.finish().unwrap()
}

/// 【归档测试】【限制样本】建立小型固定解压预算。
/// @returns 只供本地解压函数使用的请求
fn request() -> ArchiveRequest {
    ArchiveRequest {
        url: "https://example.test/archive".into(),
        destination: "snapshot".into(),
        max_bytes: 65536,
        max_unpacked_bytes: 65536,
        max_entries: 16,
        timeout_ms: 1000,
    }
}

/// 【归档测试】【安全展开】文件内容和执行位保留，发布前始终位于暂存目录。
#[test]
fn private_archive_preserves_regular_files_until_explicit_publish() {
    let root = tempfile::tempdir().unwrap();
    let directory =
        Arc::new(Dir::open_ambient_dir(root.path(), cap_std::ambient_authority()).unwrap());
    let result = unpack(
        directory.clone(),
        fixture(&[
            ("demo/PKGBUILD", b'0', b"pkgname=demo"),
            ("demo/fix.sh", b'0', b"exit 0"),
        ]),
        &request(),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert!(!directory.try_exists("snapshot").unwrap());
    assert_eq!(
        directory
            .read(format!("{}/demo/PKGBUILD", result.name))
            .unwrap(),
        b"pkgname=demo"
    );
    directory
        .rename(&result.name, &directory, "snapshot")
        .unwrap();
    drop(result);
    assert!(directory.try_exists("snapshot/demo/fix.sh").unwrap());
}

/// 【归档测试】【PAX 样本】创建指定总字节数的合法全局注释记录。
/// @param size 包含长度前缀和换行的记录字节数
/// @returns 不影响文件路径与内容的 PAX 元数据
fn pax_comment(size: usize) -> Vec<u8> {
    let prefix = format!("{size} comment=");
    let mut bytes = prefix.into_bytes();
    bytes.resize(size - 1, b'a');
    bytes.push(b'\n');
    bytes
}

/// 【归档测试】【Git 快照】合法 PAX 全局注释不生成文件，也不阻止后续源码展开。
#[test]
fn private_archive_ignores_git_pax_global_metadata() {
    let root = tempfile::tempdir().unwrap();
    let directory =
        Arc::new(Dir::open_ambient_dir(root.path(), cap_std::ambient_authority()).unwrap());
    let result = unpack(
        directory.clone(),
        fixture(&[
            (
                "pax_global_header",
                b'g',
                b"52 comment=329be2113c590046cb29858c23d9b96a8d7bd586\n",
            ),
            ("demo/", b'5', b""),
            ("demo/PKGBUILD", b'0', b"pkgname=demo"),
            ("demo/.SRCINFO", b'0', b"pkgbase = demo"),
        ]),
        &request(),
        &AtomicBool::new(false),
    )
    .unwrap();
    let contents = directory.open_dir(&result.name).unwrap();
    assert_eq!(contents.entries().unwrap().count(), 1);
    assert!(!contents.try_exists("pax_global_header").unwrap());
    assert_eq!(contents.read("demo/PKGBUILD").unwrap(), b"pkgname=demo");
    assert_eq!(contents.read("demo/.SRCINFO").unwrap(), b"pkgbase = demo");
}

/// 【归档测试】【PAX 预算】全局元数据累计最多 128 KiB，超限或条目过多时清理暂存目录。
#[test]
fn private_archive_bounds_pax_metadata_bytes_and_entries() {
    let half = pax_comment(64 * 1024);
    let excess = pax_comment(64 * 1024 + 1);
    let full = pax_comment(128 * 1024);
    for (metadata, max_entries, expected_error) in [
        (vec![full.as_slice()], 2, None),
        (
            vec![half.as_slice(), excess.as_slice()],
            3,
            Some("plugin archive exceeds metadata byte limit"),
        ),
        (
            vec![half.as_slice(), half.as_slice()],
            2,
            Some("plugin archive exceeds entry limit"),
        ),
    ] {
        let root = tempfile::tempdir().unwrap();
        let directory =
            Arc::new(Dir::open_ambient_dir(root.path(), cap_std::ambient_authority()).unwrap());
        let mut entries: Vec<_> = metadata
            .into_iter()
            .map(|bytes| ("pax_global_header", b'g', bytes))
            .collect();
        entries.push(("PKGBUILD", b'0', b"pkgname=demo"));
        let mut req = request();
        req.max_entries = max_entries;
        let result = unpack(
            directory.clone(),
            fixture(&entries),
            &req,
            &AtomicBool::new(false),
        );
        match (result, expected_error) {
            (Err(error), Some(expected)) => {
                assert!(error.to_string().contains(expected), "{error}")
            }
            (Ok(staging), None) => {
                assert_eq!(
                    directory
                        .read(format!("{}/PKGBUILD", staging.name))
                        .unwrap(),
                    b"pkgname=demo"
                );
            }
            (Err(error), None) => panic!("legal PAX metadata was rejected: {error}"),
            (Ok(_), Some(expected)) => panic!("expected rejection: {expected}"),
        }
        assert_eq!(directory.entries().unwrap().count(), 0);
    }
}

/// 【归档测试】【PAX 后续条目】跳过合法元数据后仍拒绝软链接和硬链接。
#[test]
fn private_archive_rejects_links_after_pax_global_metadata() {
    for kind in [b'1', b'2'] {
        let root = tempfile::tempdir().unwrap();
        let directory =
            Arc::new(Dir::open_ambient_dir(root.path(), cap_std::ambient_authority()).unwrap());
        let result = unpack(
            directory.clone(),
            fixture(&[
                ("pax_global_header", b'g', &pax_comment(32)),
                ("link", kind, b""),
            ]),
            &request(),
            &AtomicBool::new(false),
        );
        assert!(result
            .err()
            .unwrap()
            .to_string()
            .contains("link or unsupported"));
        assert_eq!(directory.entries().unwrap().count(), 0);
    }
}

/// 【归档测试】【越界拒绝】跨平台非法路径、链接、设备和重复文件均不留下暂存目录。
#[test]
fn private_archive_rejects_traversal_links_devices_and_duplicates() {
    for path in [
        "../escape",
        "/absolute",
        "C:/windows",
        "a\\b",
        "CON",
        "a/../escape",
    ] {
        let root = tempfile::tempdir().unwrap();
        let directory =
            Arc::new(Dir::open_ambient_dir(root.path(), cap_std::ambient_authority()).unwrap());
        assert!(
            unpack(
                directory.clone(),
                fixture(&[(path, b'0', b"bad")]),
                &request(),
                &AtomicBool::new(false)
            )
            .is_err(),
            "{path}"
        );
        assert_eq!(directory.entries().unwrap().count(), 0);
    }
    for kind in [b'1', b'2', b'3', b'4', b'6', b'S'] {
        let root = tempfile::tempdir().unwrap();
        let directory =
            Arc::new(Dir::open_ambient_dir(root.path(), cap_std::ambient_authority()).unwrap());
        assert!(unpack(
            directory.clone(),
            fixture(&[("bad", kind, b"")]),
            &request(),
            &AtomicBool::new(false)
        )
        .is_err());
        assert_eq!(directory.entries().unwrap().count(), 0);
    }
    let root = tempfile::tempdir().unwrap();
    let directory =
        Arc::new(Dir::open_ambient_dir(root.path(), cap_std::ambient_authority()).unwrap());
    assert!(unpack(
        directory.clone(),
        fixture(&[("same", b'0', b"first"), ("same", b'0', b"second")]),
        &request(),
        &AtomicBool::new(false)
    )
    .is_err());
    assert_eq!(directory.entries().unwrap().count(), 0);
}

/// 【归档测试】【资源与取消】超出展开字节、条目数或收到取消时，删除已写入暂存文件。
#[test]
fn private_archive_budget_and_cancellation_leave_no_partial_files() {
    let bytes = fixture(&[("first", b'0', b"one"), ("second", b'0', b"two")]);
    for mode in 0..3 {
        let root = tempfile::tempdir().unwrap();
        let directory =
            Arc::new(Dir::open_ambient_dir(root.path(), cap_std::ambient_authority()).unwrap());
        let mut req = request();
        if mode == 0 {
            req.max_unpacked_bytes = 4;
        }
        if mode == 1 {
            req.max_entries = 1;
        }
        assert!(unpack(
            directory.clone(),
            bytes.clone(),
            &req,
            &AtomicBool::new(mode == 2)
        )
        .is_err());
        assert_eq!(directory.entries().unwrap().count(), 0);
    }
}
