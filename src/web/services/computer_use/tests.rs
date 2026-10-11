use super::*;
use crate::paths::SaiPaths;
use flate2::write::GzEncoder;
use flate2::Compression;
use sha2::{Digest, Sha256};
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};

/// 【桌面测试】【路径集合】构造仅含数据目录的测试路径。
///
/// 参数:
/// - `root`: 临时根目录
///
/// 返回:
/// - 测试路径
fn test_paths(root: &Path) -> SaiPaths {
    SaiPaths {
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
    }
}

/// 【桌面测试】【gzip 归档】把单个文件打成 tar.gz。
///
/// 参数:
/// - `name`: 归档内路径
/// - `contents`: 文件内容
///
/// 返回:
/// - tar.gz 字节
fn gzip_tar(name: &str, contents: &[u8]) -> Vec<u8> {
    let mut tar = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_gnu();
    header.set_size(contents.len() as u64);
    header.set_mode(0o755);
    header.set_cksum();
    tar.append_data(&mut header, name, contents).unwrap();
    let packed = tar.into_inner().unwrap();
    let mut gzip = GzEncoder::new(Vec::new(), Compression::default());
    gzip.write_all(&packed).unwrap();
    gzip.finish().unwrap()
}

/// 【桌面测试】【zip 归档】把单个文件打成 zip。
///
/// 参数:
/// - `name`: 归档内路径
/// - `contents`: 文件内容
///
/// 返回:
/// - zip 字节
fn zip_bytes(name: &str, contents: &[u8]) -> Vec<u8> {
    let cursor = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(cursor);
    let options = zip::write::FileOptions::default().unix_permissions(0o755);
    zip.start_file(name, options).unwrap();
    zip.write_all(contents).unwrap();
    zip.finish().unwrap().into_inner()
}

/// 验证官方资源名覆盖 Linux、macOS 与 Windows。
#[test]
fn platform_targets_match_official_assets() {
    let linux = release::platform_target("linux", "x86_64").unwrap();
    assert_eq!(
        linux.archive_name,
        "cua-driver-rs-0.34.1-linux-x86_64-binary.tar.gz"
    );
    let mac = release::platform_target("macos", "aarch64").unwrap();
    assert_eq!(
        mac.archive_name,
        "cua-driver-rs-0.34.1-darwin-universal.tar.gz"
    );
    let windows = release::platform_target("windows", "x86_64").unwrap();
    assert_eq!(
        windows.archive_name,
        "cua-driver-rs-0.34.1-windows-x86_64.zip"
    );
    assert!(release::platform_target("freebsd", "x86_64").is_none());
}

/// 验证 SHA256SUMS 能按文件名取出摘要。
#[test]
fn checksum_parser_reads_named_archive() {
    let text = "abc123  cua-driver-rs-0.34.1-linux-x86_64-binary.tar.gz\ndef456  other.tar.gz\n";
    assert_eq!(
        release::checksum_for(text, "cua-driver-rs-0.34.1-linux-x86_64-binary.tar.gz").unwrap(),
        "abc123"
    );
    assert!(release::checksum_for(text, "missing.tar.gz").is_err());
}

/// 验证版本解析与比较。
#[test]
fn version_parser_reads_semver() {
    assert_eq!(
        process::parse_semver("cua-driver 0.34.1\n").as_deref(),
        Some("0.34.1")
    );
    assert_eq!(
        process::parse_semver("version 0.34.1-nightly.1").as_deref(),
        Some("0.34.1")
    );
    assert!(process::parse_semver("no version here").is_none());
    assert!(process::version_at_least("0.34.1", "0.34.1"));
    assert!(process::version_at_least("0.35.0", "0.34.1"));
    assert!(!process::version_at_least("0.34.0", "0.34.1"));
}

/// 验证 tar.gz 抽出驱动并忽略无关文件。
#[test]
fn tar_extract_keeps_driver_and_skips_app_bundle() {
    let mut tar = tar::Builder::new(Vec::new());
    for (name, body) in [
        (
            "cua-driver-rs-0.34.1-linux-x86_64/cua-driver",
            b"driver-bin".as_slice(),
        ),
        (
            "cua-driver-rs-0.34.1-linux-x86_64/CuaDriver.app/Contents/MacOS/CuaDriver",
            b"app".as_slice(),
        ),
        (
            "cua-driver-rs-0.34.1-linux-x86_64/wayland-helper/winrects@cua/metadata.json",
            b"{}".as_slice(),
        ),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_size(body.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, name, body).unwrap();
    }
    let packed = tar.into_inner().unwrap();
    let mut gzip = GzEncoder::new(Vec::new(), Compression::default());
    gzip.write_all(&packed).unwrap();
    let bytes = gzip.finish().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let dest = temp.path().join("out");
    let binary = archive::extract_driver(&bytes, "sample.tar.gz", &dest).unwrap();
    assert_eq!(std::fs::read_to_string(&binary).unwrap(), "driver-bin");
    assert!(!dest.join("CuaDriver.app").exists());
    assert!(dest
        .join("wayland-helper/winrects@cua/metadata.json")
        .is_file());
}

/// 验证 zip 抽出 Windows 驱动，并拒绝跳出目标目录的条目。
#[test]
fn zip_extract_rejects_parent_paths() {
    let good = zip_bytes("pkg/cua-driver.exe", b"win-driver");
    let temp = tempfile::tempdir().unwrap();
    let dest = temp.path().join("zip-out");
    let binary = archive::extract_driver(&good, "sample.zip", &dest).unwrap();
    assert_eq!(binary.file_name().unwrap(), "cua-driver.exe");

    let slip = zip_bytes("../cua-driver.exe", b"evil");
    let slip_dir = temp.path().join("slip");
    let error = archive::extract_driver(&slip, "sample.zip", &slip_dir).unwrap_err();
    assert!(error.to_string().contains("does not contain cua-driver"));
}

/// 验证安装编排校验摘要并写入数据目录。
#[test]
fn install_verifies_checksum_and_writes_binary() {
    let temp = tempfile::tempdir().unwrap();
    let paths = test_paths(temp.path());
    let target = release::current_platform().expect("linux/macos/windows test host");
    let archive = if target.archive_name.ends_with(".zip") {
        zip_bytes("pkg/cua-driver.exe", b"installed-driver")
    } else {
        gzip_tar("pkg/cua-driver", b"installed-driver")
    };
    let digest = hex::encode(Sha256::digest(&archive));
    let urls = release::release_urls(&target.archive_name);
    let sums = format!("{digest}  {}\n", target.archive_name);
    let result = install_with_fetcher(&paths, |url| {
        if url == urls.checksums {
            Ok(sums.as_bytes().to_vec())
        } else if url == urls.archive {
            Ok(archive.clone())
        } else {
            anyhow::bail!("unexpected url {url}")
        }
    })
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(&result.path).unwrap(),
        "installed-driver"
    );
    assert_eq!(result.version, PINNED_VERSION);
}

/// 验证配置的绝对路径优先于 PATH。
#[test]
fn resolve_prefers_configured_file() {
    let temp = tempfile::tempdir().unwrap();
    let configured = temp.path().join("custom-driver");
    std::fs::write(&configured, "ok").unwrap();
    let found = detect::resolve_driver_path(
        &configured.to_string_lossy(),
        temp.path().join("missing").as_path(),
    );
    assert_eq!(found.unwrap(), configured);
}

#[cfg(unix)]
#[tokio::test]
async fn probe_reads_version_from_script() {
    let temp = tempfile::tempdir().unwrap();
    let script: PathBuf = temp.path().join("cua-driver");
    std::fs::write(&script, "#!/bin/sh\necho 'cua-driver 0.34.1'\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(process::probe_version(&script).await.unwrap(), "0.34.1");
}
