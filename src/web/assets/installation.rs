use anyhow::{ensure, Context, Result};
use flate2::read::GzDecoder;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const MAX_UNPACKED_BYTES: u64 = 256 * 1024 * 1024;
const MAX_ENTRIES: usize = 20_000;

#[derive(Deserialize)]
struct Manifest {
    version: String,
}

/// 【Web 分发】【摘要校验】限制摘要为固定长度十六进制，禁止它携带路径
/// 参数: digest 为 SHA-256 文本；返回: 校验结果
pub(super) fn validate_digest(digest: &str) -> Result<()> {
    ensure!(
        digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "invalid Web SHA-256 checksum"
    );
    Ok(())
}

/// 【Web 分发】【缓存定位】读取原子更新的当前指针并校验版本和入口
/// 参数: root 为版本缓存，version 为目标版本；返回: 完整缓存路径或空
pub(super) fn current(root: &Path, version: &str) -> Option<PathBuf> {
    let installation = std::fs::read_to_string(root.join("current")).ok()?;
    let (digest, identifier) = installation.split_once('-')?;
    validate_digest(digest).ok()?;
    uuid::Uuid::parse_str(identifier).ok()?;
    let directory = root.join(installation);
    validate_bundle(&directory, version).ok()?;
    Some(directory)
}

/// 【Web 分发】【包校验】确认版本与入口文件，拒绝错误版本及残缺产物
/// 参数: directory 为解包目录，version 为后端版本；返回: 校验结果
fn validate_bundle(directory: &Path, version: &str) -> Result<()> {
    let manifest: Manifest =
        serde_json::from_slice(&std::fs::read(directory.join("sai-web.json"))?)?;
    ensure!(
        manifest.version == version,
        "Web asset version {} does not match backend {version}",
        manifest.version
    );
    ensure!(
        directory.join("index.html").is_file(),
        "Web asset bundle has no index.html"
    );
    ensure!(
        directory.join("assets").is_dir(),
        "Web asset bundle has no assets directory"
    );
    Ok(())
}

/// 【Web 分发】【原子安装】校验摘要、安全解包、检查版本，最后切换指针并保留旧目录
/// 参数: root 为缓存，version 为后端版本，bytes 为压缩包，digest 为预期摘要；返回: 新资源目录
pub(super) fn install(root: &Path, version: &str, bytes: &[u8], digest: &str) -> Result<PathBuf> {
    validate_digest(digest)?;
    ensure!(
        hex::encode(Sha256::digest(bytes)) == digest,
        "Web asset SHA-256 mismatch"
    );
    std::fs::create_dir_all(root)?;
    let stage = tempfile::Builder::new()
        .prefix(".download-")
        .tempdir_in(root)?;
    unpack(bytes, stage.path())?;
    validate_bundle(stage.path(), version)?;
    let installation = format!("{digest}-{}", uuid::Uuid::new_v4().simple());
    let directory = root.join(&installation);
    // 1. 【Web 分发】【独立目录】每次更新使用完整的新目录，修复损坏缓存且不覆盖运行中的文件
    std::fs::rename(stage.path(), &directory).context("install Web asset directory")?;
    // 2. 【Web 分发】【原子指针】持久化完整目录后替换指针，任何下载或验证失败均保留原版本
    let mut pointer = tempfile::NamedTempFile::new_in(root)?;
    pointer.write_all(installation.as_bytes())?;
    pointer.as_file().sync_all()?;
    pointer.persist(root.join("current"))?;
    Ok(directory)
}

/// 【Web 分发】【安全解包】只接受普通文件和目录，限制解压总量并拒绝越界路径与链接
/// 参数: bytes 为压缩包，directory 为临时目录；返回: 解包结果
fn unpack(bytes: &[u8], directory: &Path) -> Result<()> {
    let decoder = GzDecoder::new(bytes).take(MAX_UNPACKED_BYTES + 1);
    let mut archive = tar::Archive::new(decoder);
    let mut total = 0u64;
    for (index, entry) in archive.entries()?.enumerate() {
        ensure!(index < MAX_ENTRIES, "too many Web archive entries");
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        let text = path.to_str().context("non-UTF-8 Web archive path")?;
        ensure!(
            super::safe_relative_path(text),
            "unsafe Web archive path: {text}"
        );
        let kind = entry.header().entry_type();
        ensure!(
            kind.is_file() || kind.is_dir(),
            "Web archives cannot contain links or special files"
        );
        total = total
            .checked_add(entry.size())
            .context("Web archive size overflow")?;
        ensure!(
            total <= MAX_UNPACKED_BYTES,
            "Web archive exceeds unpacked size limit"
        );
        ensure!(
            entry.unpack_in(directory)?,
            "Web archive path escaped destination"
        );
    }
    let mut decoder = archive.into_inner();
    std::io::copy(&mut decoder, &mut std::io::sink())?;
    ensure!(
        decoder.limit() > 0,
        "Web archive exceeds decompression limit"
    );
    Ok(())
}
