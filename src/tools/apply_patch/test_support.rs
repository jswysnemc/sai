//! 补丁测试共用的临时工作目录夹具，规范化系统目录别名后再创建工作区。

use std::path::Path;
use tempfile::TempDir;

/// 【文件补丁测试】【系统目录】在系统临时目录的真实路径下创建独立工作区。
/// 参数: 无；返回: 路径已规范化且自动清理的临时工作目录。
pub(crate) fn tempdir() -> TempDir {
    tempdir_in(&std::env::temp_dir())
}

/// 【文件补丁测试】【指定目录】消除父目录的系统链接和短路径别名后创建工作区。
/// 参数: base 为工作区父目录；返回: 路径已规范化且自动清理的临时工作目录。
pub(super) fn tempdir_in(base: &Path) -> TempDir {
    // 1. 【文件补丁测试】【夹具边界】仅规范化夹具根目录，补丁目标仍接受不跟随链接的校验
    let base = base.canonicalize().unwrap();
    tempfile::tempdir_in(base).unwrap()
}
