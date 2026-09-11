use super::test_support::Fixture;
use std::os::unix::{
    ffi::OsStringExt,
    fs::{MetadataExt, PermissionsExt},
};

/// 【回收站测试】【兼容还原】标准信息保留 Unicode、百分号和原路径，现有回收站库可直接还原指定条目
/// @returns 无；不枚举系统回收站，字节、inode、权限及修改时间保持一致
#[test]
fn file_removal_trash_roundtrip_uses_standard_metadata_and_existing_restore_api() {
    let fixture = Fixture::new("图片 % #.png", b"\0\xffraw");
    std::fs::set_permissions(&fixture.path, std::fs::Permissions::from_mode(0o640)).unwrap();
    let before = std::fs::metadata(&fixture.path).unwrap();
    let (target, mut entry) = fixture.prepare();
    let (payload, info) = fixture.paths(&entry);
    let content = std::fs::read_to_string(&info).unwrap();
    assert!(content.starts_with("[Trash Info]\n"));
    let encoded = content
        .lines()
        .find_map(|line| line.strip_prefix("Path="))
        .unwrap();
    let original = std::path::PathBuf::from(std::ffi::OsString::from_vec(
        urlencoding::decode_binary(encoded.as_bytes()).into_owned(),
    ));
    assert_eq!(original, fixture.path.canonicalize().unwrap());
    assert!(encoded.contains("%25"));
    let date = content
        .lines()
        .find_map(|line| line.strip_prefix("DeletionDate="))
        .unwrap();
    chrono::NaiveDateTime::parse_from_str(date, "%Y-%m-%dT%H:%M:%S").unwrap();
    assert!(fixture.path.exists());
    assert!(!payload.exists());
    assert!(entry.commit(&target).unwrap());
    drop(entry);
    assert!(!fixture.path.exists());
    assert_eq!(std::fs::read(&payload).unwrap(), b"\0\xffraw");
    let after = std::fs::metadata(&payload).unwrap();
    assert_eq!(
        (
            after.dev(),
            after.ino(),
            after.mode(),
            after.modified().unwrap()
        ),
        (
            before.dev(),
            before.ino(),
            before.mode(),
            before.modified().unwrap()
        )
    );
    let item = ::trash::TrashItem {
        id: info.as_os_str().to_os_string(),
        name: original.file_name().unwrap().to_os_string(),
        original_parent: original.parent().unwrap().to_path_buf(),
        time_deleted: chrono::Local::now().timestamp(),
    };
    ::trash::os_limited::restore_all([item]).unwrap();
    assert_eq!(std::fs::read(&fixture.path).unwrap(), b"\0\xffraw");
    assert!(!payload.exists());
    assert!(!info.exists());
}

/// 【回收站测试】【重复原名】同一路径反复回收必须保留不同版本，禁止覆盖任何旧条目
/// @returns 无；两个独立条目具有相同原位置和不同正文
#[test]
fn file_removal_trash_keeps_repeated_original_names_without_overwrite() {
    let fixture = Fixture::new("same.png", b"first");
    let mut paths = Vec::new();
    for bytes in [b"first".as_slice(), b"second"] {
        std::fs::write(&fixture.path, bytes).unwrap();
        let (target, mut entry) = fixture.prepare();
        paths.push(fixture.paths(&entry));
        assert!(entry.commit(&target).unwrap());
    }
    assert_ne!(paths[0].0, paths[1].0);
    assert_eq!(std::fs::read(&paths[0].0).unwrap(), b"first");
    assert_eq!(std::fs::read(&paths[1].0).unwrap(), b"second");
    assert!(paths.iter().all(|(_, info)| info.is_file()));
}

/// 【回收站测试】【迟到准备】释放未提交结果只能清理预留信息，不能删除或移动源文件
/// @returns 无；目录可以保留为空，正文始终保持原样
#[test]
fn file_removal_late_trash_preparation_only_cleans_its_information() {
    let fixture = Fixture::new("cancelled", b"keep");
    let (_, entry) = fixture.prepare();
    let (payload, info) = fixture.paths(&entry);
    assert!(info.exists());
    drop(entry);
    assert_eq!(std::fs::read(&fixture.path).unwrap(), b"keep");
    assert!(!payload.exists());
    assert!(!info.exists());
}

/// 【回收站测试】【目标冲突】原子移动碰到占用名称必须失败，不能永久删除源文件或覆盖旧内容
/// @returns 无；失败只移除本次创建的信息文件
#[test]
fn file_removal_trash_publication_collision_preserves_both_files() {
    let fixture = Fixture::new("collision", b"source");
    let (target, mut entry) = fixture.prepare();
    let (payload, info) = fixture.paths(&entry);
    std::fs::write(&payload, b"existing").unwrap();
    assert!(entry.commit(&target).is_err());
    drop(entry);
    assert_eq!(std::fs::read(&fixture.path).unwrap(), b"source");
    assert_eq!(std::fs::read(&payload).unwrap(), b"existing");
    assert!(!info.exists());
}

/// 【回收站测试】【提交前变化】提交前目标缺失或替换时不得移动后来出现的文件
/// @returns 无；缺失返回 false，替换报错且保存全部外部变更
#[test]
fn file_removal_trash_rechecks_missing_and_replaced_sources() {
    for replaced in [false, true] {
        let fixture = Fixture::new("changed", b"original");
        let (target, mut entry) = fixture.prepare();
        let (payload, info) = fixture.paths(&entry);
        std::fs::rename(&fixture.path, fixture.root.path().join("saved")).unwrap();
        if replaced {
            std::fs::write(&fixture.path, b"replacement").unwrap();
        }
        let result = entry.commit(&target);
        if replaced {
            assert!(result.is_err());
        } else {
            assert!(!result.unwrap());
        }
        drop(entry);
        assert!(!payload.exists());
        assert!(!info.exists());
        assert_eq!(
            std::fs::read(fixture.root.path().join("saved")).unwrap(),
            b"original"
        );
        if replaced {
            assert_eq!(std::fs::read(&fixture.path).unwrap(), b"replacement");
        }
    }
}
