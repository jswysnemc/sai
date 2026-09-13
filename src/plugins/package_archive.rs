use anyhow::{bail, Context, Result};
use flate2::{Compression, GzBuilder};
use sai_plugin_runtime::PluginPackage;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// 【插件分发】【打包结果】描述已完整发布的源码归档，不包含用户配置或授权
#[derive(Debug, Serialize)]
pub(crate) struct PackagedPlugin {
    pub id: String,
    pub version: String,
    pub api_version: u32,
    pub path: PathBuf,
    pub bytes: u64,
    pub sha256: String,
    pub files: Vec<String>,
}

/// 【插件分发】【创建归档】校验固定源码快照并发布标准 tar.gz，不覆盖已有路径
/// @param directory 源码包目录；output 为可选输出，缺省为当前目录下的 ID-版本.tar.gz
/// @returns 归档绝对路径、大小、摘要及文件列表，失败时清理暂存文件
pub(crate) fn pack(directory: &Path, output: Option<&Path>) -> Result<PackagedPlugin> {
    // 1. 【插件分发】【快照校验】只使用已读取的清单与 Lua，不再次读取可变源码目录
    let package = PluginPackage::from_directory(directory)?;
    super::management::inspect_package(package.clone())?;
    let manifest = package.manifest_json()?;
    let default_name = PathBuf::from(format!(
        "{}-{}.tar.gz",
        package.manifest.id, package.manifest.version
    ));
    let destination = destination(output.unwrap_or(&default_name))?;

    // 2. 【插件分发】【同目录暂存】完整压缩并同步后才发布，已有文件和链接都不覆盖
    let mut staging = tempfile::Builder::new()
        .prefix(".sai-plugin-pack-")
        .tempfile_in(
            destination
                .parent()
                .context("archive output has no parent")?,
        )?;
    let files = write_archive(staging.as_file_mut(), &package, &manifest)?;
    staging.as_file().sync_all()?;
    let bytes = staging.as_file().metadata()?.len();
    let sha256 = sha256(staging.as_file_mut())?;
    staging
        .persist_noclobber(&destination)
        .map_err(|error| error.error)
        .with_context(|| {
            format!(
                "publish plugin archive without replacing {}",
                destination.display()
            )
        })?;
    Ok(PackagedPlugin {
        id: package.manifest.id,
        version: package.manifest.version,
        api_version: package.manifest.api_version,
        path: destination,
        bytes,
        sha256,
        files,
    })
}

/// 【插件分发】【输出路径】解析归档文件名并创建父目录，拒绝已有路径
/// @param output 用户路径或默认文件名，必须是 UTF-8 的 tar.gz 或 tgz 路径
/// @returns 规范父目录下的绝对输出路径，不创建最终文件
fn destination(output: &Path) -> Result<PathBuf> {
    let filename = output
        .file_name()
        .and_then(|name| name.to_str())
        .context("plugin archive output must have a UTF-8 filename")?;
    if !filename.ends_with(".tar.gz") && !filename.ends_with(".tgz") {
        bail!("plugin archive output must end with .tar.gz or .tgz");
    }
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).context("create plugin archive output directory")?;
    let destination = parent.canonicalize()?.join(filename);
    destination
        .to_str()
        .context("plugin archive output path must be UTF-8")?;
    match std::fs::symlink_metadata(&destination) {
        Ok(_) => bail!(
            "plugin archive output already exists: {}",
            destination.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(destination),
        Err(error) => Err(error).context("inspect plugin archive output"),
    }
}

/// 【插件分发】【固定归档】按固定顺序写入规范清单与 Lua，时间和用户元数据不随机器变化
/// @param writer 暂存输出；package 为固定源码快照；manifest 为已经检查大小的清单 JSON
/// @returns 归档中的普通文件路径列表，压缩完成前不交付结果
fn write_archive(
    writer: impl Write,
    package: &PluginPackage,
    manifest: &[u8],
) -> Result<Vec<String>> {
    let gzip = GzBuilder::new()
        .mtime(0)
        .operating_system(255)
        .write(writer, Compression::default());
    let mut archive = tar::Builder::new(gzip);
    let mut files = Vec::with_capacity(package.sources().len() + 1);
    files.push(append(
        &mut archive,
        &package.manifest.id,
        "sai-plugin.json",
        manifest,
    )?);
    for (path, source) in package.sources() {
        files.push(append(
            &mut archive,
            &package.manifest.id,
            path,
            source.as_bytes(),
        )?);
    }
    archive.into_inner()?.finish()?;
    Ok(files)
}

/// 【插件分发】【普通文件】为快照字节建立固定权限和零时间的归档条目
/// @param archive 归档写入器；id 为规范插件标识；relative 为规范包内路径；bytes 为内容
/// @returns 含插件根目录的归档路径，不读取磁盘元数据或链接
fn append<W: Write>(
    archive: &mut tar::Builder<W>,
    id: &str,
    relative: &str,
    bytes: &[u8],
) -> Result<String> {
    let path = format!("{id}/{relative}");
    let mut header = tar::Header::new_gnu();
    header.set_entry_type(tar::EntryType::Regular);
    header.set_mode(0o644);
    header.set_uid(0);
    header.set_gid(0);
    header.set_mtime(0);
    header.set_size(bytes.len() as u64);
    header.set_cksum();
    archive.append_data(&mut header, &path, bytes)?;
    Ok(path)
}

/// 【插件分发】【文件摘要】从已完成的暂存文件计算全部压缩字节的 SHA-256
/// @param file 可定位的暂存归档文件
/// @returns 小写十六进制摘要，读取失败时不发布最终归档
fn sha256(file: &mut std::fs::File) -> Result<String> {
    file.seek(SeekFrom::Start(0))?;
    let mut digest = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            return Ok(hex::encode(digest.finalize()));
        }
        digest.update(&buffer[..count]);
    }
}
