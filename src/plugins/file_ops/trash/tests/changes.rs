use super::{test_support::Fixture, *};

/// 【回收站测试】【移动期间替换】模拟复核之后文件被替换为目录，必须无覆盖还原且不遍历内容
/// @returns 无；原文件、替换目录与目录正文均保留
#[test]
fn file_removal_trash_restores_a_nonregular_entry_captured_during_commit() {
    let fixture = Fixture::new("changed", b"original");
    let (target, mut entry) = fixture.prepare();
    let (payload, info) = fixture.paths(&entry);
    std::fs::rename(&fixture.path, fixture.root.path().join("saved")).unwrap();
    std::fs::create_dir(&fixture.path).unwrap();
    std::fs::write(fixture.path.join("child"), b"child").unwrap();
    let error = entry.move_and_verify(&target).unwrap_err();
    assert!(format!("{error:#}").contains("entry restored"));
    drop(entry);
    assert!(!payload.exists());
    assert!(!info.exists());
    assert_eq!(std::fs::read(fixture.path.join("child")).unwrap(), b"child");
    assert_eq!(
        std::fs::read(fixture.root.path().join("saved")).unwrap(),
        b"original"
    );
}

/// 【回收站测试】【还原冲突】原名重新占用时保留可恢复条目与信息，不覆盖竞争方文件
/// @returns 无；错误明确说明回收站保留状态
#[test]
fn file_removal_trash_rollback_collision_retains_the_recovery_entry() {
    let fixture = Fixture::new("occupied", b"captured");
    let (target, mut entry) = fixture.prepare();
    let (payload, info) = fixture.paths(&entry);
    rename_noreplace(
        &target.directory,
        &target.name,
        &entry.reservation.directories.files,
        OsStr::new(&entry.reservation.name),
    )
    .unwrap();
    entry.reservation.retained = true;
    std::fs::write(&fixture.path, b"new occupant").unwrap();
    let error = entry
        .restore_after_change(&target, anyhow::anyhow!("fixture concurrent change"))
        .unwrap_err();
    assert!(format!("{error:#}").contains("item retained in system trash"));
    drop(entry);
    assert_eq!(std::fs::read(&fixture.path).unwrap(), b"new occupant");
    assert_eq!(std::fs::read(&payload).unwrap(), b"captured");
    assert!(info.exists());
}

/// 【回收站测试】【目录替换】准备后回收站目录被替换成链接时，提交不得把源文件送到脱离标准位置的目录
/// @returns 无；原文件及外部目录保持不变，预留信息仍能通过固定句柄清理
#[test]
fn file_removal_trash_rejects_directory_replacement_before_commit() {
    let fixture = Fixture::new("directory-race", b"source");
    let (target, mut entry) = fixture.prepare();
    let (_, info) = fixture.paths(&entry);
    let files = fixture.data.join("Trash/files");
    let saved = fixture.root.path().join("former-files");
    let outside = fixture.root.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    std::fs::write(outside.join("keep"), b"outside").unwrap();
    std::fs::rename(&files, &saved).unwrap();
    std::os::unix::fs::symlink(&outside, &files).unwrap();
    assert!(entry.commit(&target).is_err());
    drop(entry);
    assert_eq!(std::fs::read(&fixture.path).unwrap(), b"source");
    assert_eq!(std::fs::read(outside.join("keep")).unwrap(), b"outside");
    assert_eq!(std::fs::read_dir(&saved).unwrap().count(), 0);
    assert!(!info.exists());
}

/// 【回收站测试】【信息替换】准备后信息名称被替换时拒绝移动，也不能清理替换者的新文件
/// @returns 无；源文件、旧信息与替换信息均保持原样
#[test]
fn file_removal_trash_rejects_replaced_information_without_deleting_the_replacement() {
    let fixture = Fixture::new("information-race", b"source");
    let (target, mut entry) = fixture.prepare();
    let (payload, info) = fixture.paths(&entry);
    std::fs::rename(&info, fixture.root.path().join("saved-info")).unwrap();
    std::fs::write(&info, b"unrelated information").unwrap();
    assert!(entry.commit(&target).is_err());
    drop(entry);
    assert_eq!(std::fs::read(&fixture.path).unwrap(), b"source");
    assert_eq!(std::fs::read(&info).unwrap(), b"unrelated information");
    assert!(!payload.exists());
}
