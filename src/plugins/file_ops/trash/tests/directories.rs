use super::{test_support::Fixture, *};
use std::os::unix::fs::{MetadataExt, PermissionsExt};

/// 【回收站测试】【私有权限】已有回收站或子目录权限过宽时拒绝，不擅自修改权限
/// @returns 无；拒绝发生在文件移动和信息创建之前
#[test]
fn file_removal_trash_rejects_public_directory_permissions() {
    for component in ["Trash", "Trash/files", "Trash/info"] {
        let fixture = Fixture::new("permissions", b"keep");
        std::fs::create_dir_all(fixture.data.join("Trash/files")).unwrap();
        std::fs::create_dir_all(fixture.data.join("Trash/info")).unwrap();
        for path in ["Trash", "Trash/files", "Trash/info"] {
            std::fs::set_permissions(
                fixture.data.join(path),
                std::fs::Permissions::from_mode(if path == component { 0o755 } else { 0o700 }),
            )
            .unwrap();
        }
        let target = Target::open(fixture.path.canonicalize().unwrap())
            .unwrap()
            .unwrap();
        assert!(prepare_at(&target, &fixture.data, &AtomicBool::new(false)).is_err());
        assert_eq!(std::fs::read(&fixture.path).unwrap(), b"keep");
        assert_eq!(
            std::fs::metadata(fixture.data.join(component))
                .unwrap()
                .mode()
                & 0o777,
            0o755
        );
        assert_eq!(
            std::fs::read_dir(fixture.data.join("Trash/info"))
                .unwrap()
                .count(),
            0
        );
    }
}

/// 【回收站测试】【目录链接】标准回收站根与两个子目录不得通过链接重定向
/// @returns 无；外部目录只保留原有标记文件
#[test]
fn file_removal_trash_rejects_linked_trash_directories() {
    for component in ["Trash", "Trash/files", "Trash/info"] {
        let fixture = Fixture::new("links", b"keep");
        let outside = fixture.root.path().join("outside");
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(outside.join("marker"), b"outside").unwrap();
        let linked = fixture.data.join(component);
        std::fs::create_dir_all(linked.parent().unwrap()).unwrap();
        if component != "Trash" {
            std::fs::set_permissions(
                fixture.data.join("Trash"),
                std::fs::Permissions::from_mode(0o700),
            )
            .unwrap();
        }
        std::os::unix::fs::symlink(&outside, &linked).unwrap();
        let target = Target::open(fixture.path.canonicalize().unwrap())
            .unwrap()
            .unwrap();
        assert!(prepare_at(&target, &fixture.data, &AtomicBool::new(false)).is_err());
        assert_eq!(std::fs::read(&fixture.path).unwrap(), b"keep");
        assert_eq!(std::fs::read_dir(&outside).unwrap().count(), 1);
    }
}

/// 【回收站测试】【取消前置】已经撤销的工作不创建回收站或改变源文件
/// @returns 无；所有文件准备均发生在取消检查之后
#[test]
fn file_removal_cancelled_trash_preparation_has_no_filesystem_effect() {
    let fixture = Fixture::new("cancelled", b"keep");
    let target = Target::open(fixture.path.canonicalize().unwrap())
        .unwrap()
        .unwrap();
    assert!(prepare_at(&target, &fixture.data, &AtomicBool::new(true)).is_err());
    assert!(!fixture.data.exists());
    assert_eq!(std::fs::read(&fixture.path).unwrap(), b"keep");
}

/// 【回收站测试】【跨设备拒绝】用户数据目录位于另一文件系统时拒绝，不复制或删除源文件
/// @returns 无；仅在可用的独立内存文件系统上执行跨设备断言
#[test]
fn file_removal_trash_never_falls_back_to_cross_filesystem_copy_and_delete() {
    let fixture = Fixture::new("cross-device", b"keep");
    let Ok(other) = tempfile::tempdir_in("/dev/shm") else {
        return;
    };
    if std::fs::metadata(other.path()).unwrap().dev()
        == std::fs::metadata(&fixture.path).unwrap().dev()
    {
        return;
    }
    let target = Target::open(fixture.path.canonicalize().unwrap())
        .unwrap()
        .unwrap();
    let error = match prepare_at(&target, other.path(), &AtomicBool::new(false)) {
        Ok(_) => panic!("cross-device trash unexpectedly prepared"),
        Err(error) => error,
    };
    assert!(format!("{error:#}").contains("same filesystem"));
    assert!(!other.path().join("Trash").exists());
    assert_eq!(std::fs::read(&fixture.path).unwrap(), b"keep");
}

/// 【回收站测试】【回收站自身】不能再次回收已经位于系统回收站中的文件
/// @returns 无；已有条目不移动也不创建新的还原信息
#[test]
fn file_removal_trash_cannot_retrash_existing_trash_files() {
    let fixture = Fixture::new("inside", b"keep");
    let inside = fixture.data.join("Trash/files/already");
    std::fs::create_dir_all(inside.parent().unwrap()).unwrap();
    std::fs::write(&inside, b"existing").unwrap();
    let target = Target::open(inside.canonicalize().unwrap())
        .unwrap()
        .unwrap();
    assert!(prepare_at(&target, &fixture.data, &AtomicBool::new(false)).is_err());
    assert_eq!(std::fs::read(&inside).unwrap(), b"existing");
    assert!(!fixture.data.join("Trash/info").exists());
}
