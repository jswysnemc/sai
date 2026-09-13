use crate::manifest::validate_relative_file;
use crate::PluginManifest;
use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

const MAX_SOURCE_BYTES: usize = 4 * 1024 * 1024;
const MAX_SOURCE_FILES: usize = 128;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;

/// 【插件】【源码快照】加载后固定源码，运行中不重新读取可变磁盘文件。
#[derive(Clone, Debug)]
pub struct PluginPackage {
    pub manifest: PluginManifest,
    pub(crate) sources: Arc<BTreeMap<String, String>>,
}

impl PluginPackage {
    /// 【插件】【源码读取】提供加载时的只读源码快照，供宿主安装和比较版本。
    /// @returns 包内规范路径到 UTF-8 源码的映射，不读取磁盘
    pub fn sources(&self) -> &BTreeMap<String, String> {
        &self.sources
    }

    /// 【插件】【清单序列化】输出能再次通过目录加载上限的规范 JSON，保留末尾换行
    /// @returns 格式化 JSON；接近大小上限时使用紧凑编码，仍超限则返回错误
    pub fn manifest_json(&self) -> Result<Vec<u8>> {
        self.manifest.validate()?;
        let mut bytes = serde_json::to_vec_pretty(&self.manifest)?;
        bytes.push(b'\n');
        // 1. 【插件】【编码收窄】排版空白不能使有效清单在保存后变成不可加载的文件
        if bytes.len() as u64 > MAX_MANIFEST_BYTES {
            bytes = serde_json::to_vec(&self.manifest)?;
            bytes.push(b'\n');
        }
        if bytes.len() as u64 > MAX_MANIFEST_BYTES {
            bail!("serialized plugin manifest exceeds 64 KiB");
        }
        Ok(bytes)
    }

    /// 【插件】【内置加载】从已取得的清单和源码构造插件快照。
    /// @param manifest 已解析清单；sources 为包内路径到 Lua 源码的映射
    /// @returns 已验证的插件包，缺失入口或超限返回错误
    pub fn new(manifest: PluginManifest, sources: BTreeMap<String, String>) -> Result<Self> {
        manifest.validate()?;
        if sources.len() > MAX_SOURCE_FILES
            || sources.values().map(String::len).sum::<usize>() > MAX_SOURCE_BYTES
        {
            bail!("plugin source package exceeds size limits");
        }
        for path in sources.keys() {
            validate_relative_file(path)?;
            if !path.ends_with(".lua") {
                bail!("plugin source is not Lua: {path}");
            }
        }
        if !sources.contains_key(&manifest.entry) {
            bail!("plugin entry file not found: {}", manifest.entry);
        }
        Ok(Self {
            manifest,
            sources: Arc::new(sources),
        })
    }

    /// 【插件】【目录加载】从插件目录读取清单和 Lua 源码，拒绝符号链接。
    /// @param root 包含 sai-plugin.json 的目录
    /// @returns 自包含源码快照，后续编辑不会改变已启动的插件
    pub fn from_directory(root: &Path) -> Result<Self> {
        let metadata = std::fs::symlink_metadata(root).context("read plugin directory")?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            bail!("plugin root must be a real directory");
        }
        let manifest_path = root.join("sai-plugin.json");
        let metadata = std::fs::symlink_metadata(&manifest_path).context("read sai-plugin.json")?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || metadata.len() > MAX_MANIFEST_BYTES
        {
            bail!("plugin manifest must be a regular file of at most 64 KiB");
        }
        let manifest = PluginManifest::parse(&std::fs::read_to_string(&manifest_path)?)?;
        let mut sources = BTreeMap::new();
        let mut total = 0;
        read_sources(root, root, &mut sources, &mut total, 0)?;
        Self::new(manifest, sources)
    }
}

/// 【插件】【目录扫描】有界读取 Lua 文件，阻止目录遍历和无限递归。
/// @param root 为插件根目录；directory 为当前目录；sources、total 累积源码；depth 为深度
/// @returns 读取结果，不执行插件代码
fn read_sources(
    root: &Path,
    directory: &Path,
    sources: &mut BTreeMap<String, String>,
    total: &mut usize,
    depth: usize,
) -> Result<()> {
    if depth > 12 {
        bail!("plugin directory nesting exceeds 12 levels");
    }
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            bail!("plugin packages must not contain symbolic links");
        }
        if kind.is_dir() {
            if entry.file_name() != ".git" {
                read_sources(root, &path, sources, total, depth + 1)?;
            }
            continue;
        }
        if !kind.is_file() || path.extension().and_then(|value| value.to_str()) != Some("lua") {
            continue;
        }
        let size = entry.metadata()?.len();
        if sources.len() >= MAX_SOURCE_FILES
            || size > MAX_SOURCE_BYTES.saturating_sub(*total) as u64
        {
            bail!("plugin source package exceeds size limits");
        }
        let relative = relative_source_path(root, &path)?;
        let source = std::fs::read_to_string(&path)?;
        *total += source.len();
        sources.insert(relative, source);
    }
    Ok(())
}

/// 【插件】【源码路径】按真实路径组件生成包内名称，不改写文件名中的字符
/// @param root 插件根目录；path 为扫描得到的 Lua 文件路径
/// @returns 通过跨平台规则校验的相对路径，非法名称不会合并到其他源码
fn relative_source_path(root: &Path, path: &Path) -> Result<String> {
    let relative = path
        .strip_prefix(root)?
        .iter()
        .map(|component| component.to_str().context("plugin file path is not UTF-8"))
        .collect::<Result<Vec<_>>>()?
        .join("/");
    validate_relative_file(&relative)
        .with_context(|| format!("invalid plugin source path: {relative:?}"))?;
    Ok(relative)
}
