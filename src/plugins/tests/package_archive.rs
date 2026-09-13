use crate::plugins;
use flate2::read::GzDecoder;
use sai_plugin_runtime::PluginPackage;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};

/// 【插件打包测试】【源码样本】生成带嵌套模块及不应打包文件的普通插件
/// @param root 临时目录；name 为源码目录名称
/// @returns 可独立通过检查的源码目录
fn source(root: &Path, name: &str) -> PathBuf {
    let directory = root.join(name);
    plugins::scaffold(&directory, "pack-example").unwrap();
    std::fs::create_dir(directory.join("nested")).unwrap();
    std::fs::write(
        directory.join("nested/value.lua"),
        "return 'nested value'\n",
    )
    .unwrap();
    std::fs::write(directory.join("nested/辅助.lua"), "return 'UTF-8 path'\n").unwrap();
    let long_directory = directory.join("nested").join("long".repeat(35));
    std::fs::create_dir(&long_directory).unwrap();
    std::fs::write(long_directory.join("value.lua"), "return 'long path'\n").unwrap();
    std::fs::write(directory.join("README.md"), "Release notes stay separate\n").unwrap();
    std::fs::write(directory.join(".env"), "TOKEN=fixture-only\n").unwrap();
    std::fs::write(directory.join("settings.json"), "{\"fixture\":true}\n").unwrap();
    std::fs::create_dir(directory.join(".git")).unwrap();
    std::fs::write(directory.join(".git/ignored.lua"), "not a Lua module\n").unwrap();
    directory
}

/// 【插件打包测试】【条目读取】检查标准压缩包中的真实文件和固定元数据
/// @param path 已发布的归档路径
/// @returns 规范归档路径到完整字节的映射
fn contents(path: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let gzip = GzDecoder::new(std::fs::File::open(path).unwrap());
    let mut archive = tar::Archive::new(gzip);
    let mut files = BTreeMap::new();
    for entry in archive.entries().unwrap() {
        let mut entry = entry.unwrap();
        let path = entry.path().unwrap().into_owned();
        assert!(entry.header().entry_type().is_file());
        assert_eq!(entry.header().mode().unwrap(), 0o644);
        assert_eq!(entry.header().uid().unwrap(), 0);
        assert_eq!(entry.header().gid().unwrap(), 0);
        assert_eq!(entry.header().mtime().unwrap(), 0);
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        assert!(files.insert(path, bytes).is_none());
    }
    files
}

/// 【插件打包测试】【可复现快照】只包含清单和 Lua，源码目录位置及文件时间不影响字节
/// @returns 无，摘要覆盖整个归档，解压后使用正式加载器读取同一内容
#[test]
fn packed_sources_are_deterministic_minimal_and_reloadable() {
    let root = tempfile::tempdir().unwrap();
    let first = source(root.path(), "first");
    let second = source(root.path(), "second");
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(second.join("init.lua"))
        .unwrap();
    file.set_times(
        std::fs::FileTimes::new()
            .set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(1)),
    )
    .unwrap();
    drop(file);
    let left = plugins::pack(&first, Some(&root.path().join("first.tar.gz"))).unwrap();
    let right = plugins::pack(&second, Some(&root.path().join("second.tgz"))).unwrap();
    let bytes = std::fs::read(&left.path).unwrap();
    assert_eq!(bytes, std::fs::read(&right.path).unwrap());
    assert_eq!(left.bytes, bytes.len() as u64);
    assert_eq!(left.sha256, hex::encode(Sha256::digest(&bytes)));
    assert_eq!(left.sha256, right.sha256);
    assert_eq!(left.id, "pack-example");
    assert_eq!(left.version, "0.1.0");
    assert_eq!(left.api_version, 1);
    let files = contents(&left.path);
    assert_eq!(files.len(), 5);
    assert_eq!(
        left.files
            .iter()
            .map(PathBuf::from)
            .collect::<std::collections::BTreeSet<_>>(),
        files.keys().cloned().collect()
    );
    assert_eq!(
        files[Path::new("pack-example/nested/value.lua")],
        b"return 'nested value'\n"
    );
    assert!(files.contains_key(Path::new("pack-example/nested/辅助.lua")));
    let extracted = root.path().join("unpacked");
    tar::Archive::new(GzDecoder::new(bytes.as_slice()))
        .unpack(&extracted)
        .unwrap();
    let restored = PluginPackage::from_directory(&extracted.join("pack-example")).unwrap();
    let original = PluginPackage::from_directory(&first).unwrap();
    assert_eq!(restored.sources(), original.sources());
    assert_eq!(
        serde_json::to_value(restored.manifest).unwrap(),
        serde_json::to_value(original.manifest).unwrap()
    );
    assert_eq!(
        plugins::validate_package(&extracted.join("pack-example"))
            .unwrap()
            .commands
            .len(),
        1
    );
}

/// 【插件打包测试】【初始化失败】注册检查失败前不得创建目标或输出目录
/// @returns 无，失败仍保留原源码
#[test]
fn failed_initialization_does_not_create_an_archive() {
    let root = tempfile::tempdir().unwrap();
    let source = source(root.path(), "source");
    std::fs::write(
        source.join("init.lua"),
        "error('fixture initialization failure')\n",
    )
    .unwrap();
    let output = root.path().join("absent/release.tar.gz");
    let error = plugins::pack(&source, Some(&output)).unwrap_err();
    assert!(format!("{error:#}").contains("fixture initialization failure"));
    assert!(!output.parent().unwrap().exists());
}

/// 【插件打包测试】【已有输出】拒绝覆盖文件或目录，不截断原数据且不保留暂存文件
/// @returns 无，重复发布必须换用新路径或新版本
#[test]
fn existing_archive_outputs_are_never_replaced() {
    let root = tempfile::tempdir().unwrap();
    let source = source(root.path(), "source");
    let output = root.path().join("existing.tar.gz");
    std::fs::write(&output, "original archive").unwrap();
    assert!(plugins::pack(&source, Some(&output)).is_err());
    assert_eq!(std::fs::read(&output).unwrap(), b"original archive");
    let directory = root.path().join("directory.tgz");
    std::fs::create_dir(&directory).unwrap();
    assert!(plugins::pack(&source, Some(&directory)).is_err());
    assert!(directory.is_dir());
    assert!(std::fs::read_dir(root.path()).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".sai-plugin-pack-")
    }));
}

/// 【插件打包测试】【输出格式】非归档文件名在创建输出目录之前失败
/// @returns 无，格式错误不产生目标文件
#[test]
fn archive_output_requires_the_documented_extension() {
    let root = tempfile::tempdir().unwrap();
    let source = source(root.path(), "source");
    let output = root.path().join("absent/output.lua");
    assert!(plugins::pack(&source, Some(&output)).is_err());
    assert!(!output.parent().unwrap().exists());
}

/// 【插件打包测试】【并发发布】多个发布者争用同一路径时只能完整发布一次
/// @returns 无，最终归档有效且未留下未完成的暂存文件
#[test]
fn competing_publishers_leave_one_complete_archive() {
    let root = tempfile::tempdir().unwrap();
    let source = source(root.path(), "source");
    let output = root.path().join("release.tar.gz");
    let barrier = Arc::new(Barrier::new(2));
    let workers = (0..2)
        .map(|_| {
            let source = source.clone();
            let output = output.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                plugins::pack(&source, Some(&output))
            })
        })
        .collect::<Vec<_>>();
    let results = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(contents(&output).len(), 5);
    assert!(std::fs::read_dir(root.path()).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".sai-plugin-pack-")
    }));
}

/// 【插件打包测试】【符号链接】输出路径是链接时不覆盖链接本身或其指向的数据
/// @returns 无，目标文件和原链接保持不变
#[cfg(unix)]
#[test]
fn archive_outputs_do_not_follow_symbolic_links() {
    let root = tempfile::tempdir().unwrap();
    let source = source(root.path(), "source");
    let target = root.path().join("original");
    std::fs::write(&target, "original data").unwrap();
    let output = root.path().join("linked.tgz");
    std::os::unix::fs::symlink(&target, &output).unwrap();
    assert!(plugins::pack(&source, Some(&output)).is_err());
    assert!(output.is_symlink());
    assert_eq!(std::fs::read(&target).unwrap(), b"original data");
}
