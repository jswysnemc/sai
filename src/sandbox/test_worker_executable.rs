use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// 【Windows命令】【测试启动器】定位 Cargo 构建的正式程序，不使用工作目录或 PATH。
/// 参数：`current` 为测试程序的绝对路径；返回正式启动器路径，缺失时返回构建提示。
pub(super) fn resolve(current: &Path) -> Result<PathBuf> {
    anyhow::ensure!(
        current.is_absolute(),
        "test executable path must be absolute"
    );
    // 1. 【Windows命令】【测试启动器】新版 Cargo 使用 build 子目录，不能依赖固定的父目录层数
    let artifacts = current
        .parent()
        .context("test executable directory")?
        .ancestors()
        .find(|dir| {
            matches!(
                dir.file_name().and_then(|name| name.to_str()),
                Some("deps" | "build")
            )
        })
        .context("test executable is not in a Cargo deps or build directory")?;
    // 2. 【Windows命令】【测试启动器】只读取构建配置目录中的正式程序，不向其他祖先查找同名文件
    let executable = artifacts
        .parent()
        .context("Cargo build profile directory")?
        .join("sai.exe");
    anyhow::ensure!(
        executable.is_file(),
        "build sai.exe before running Windows command tests: {}",
        executable.display()
    );
    Ok(executable)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【Windows命令】【路径夹具】创建隔离的 Cargo 输出目录及正式启动器。
    /// 参数：`profile` 为构建配置目录，`layout` 为测试程序相对路径；返回临时目录及程序路径。
    fn fixture(profile: &str, layout: &str) -> (tempfile::TempDir, PathBuf, PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let profile = root.path().join(profile);
        let current = profile.join(layout);
        let worker = profile.join("sai.exe");
        std::fs::create_dir_all(current.parent().unwrap()).unwrap();
        std::fs::write(&current, b"test fixture").unwrap();
        std::fs::write(&worker, b"worker fixture").unwrap();
        (root, current, worker)
    }

    /// 【Windows命令】【传统布局】验证 deps 目录中的测试程序定位到正式启动器。
    /// 参数：无；返回：无。
    #[test]
    fn resolves_deps_layout() {
        let (_root, current, worker) = fixture("target/debug", "deps/sai-hash.exe");
        assert_eq!(resolve(&current).unwrap(), worker);
    }

    /// 【Windows命令】【构建布局】验证旧版 build 输出目录中的测试程序定位。
    /// 参数：无；返回：无。
    #[test]
    fn resolves_legacy_build_layout() {
        let (_root, current, worker) = fixture("target/debug", "build/sai-hash/out/sai-hash.exe");
        assert_eq!(resolve(&current).unwrap(), worker);
    }

    /// 【Windows命令】【新版布局】验证 Rust 1.100 使用的分层 build 输出目录。
    /// 参数：无；返回：无。
    #[test]
    fn resolves_nested_build_layout() {
        let (_root, current, worker) = fixture("target/debug", "build/sai/hash/out/sai-hash.exe");
        assert_eq!(resolve(&current).unwrap(), worker);
    }

    /// 【Windows命令】【交叉构建】验证自定义目标目录、目标三元组及 release 配置。
    /// 参数：无；返回：无。
    #[test]
    fn resolves_custom_target_and_release_profile() {
        let (_root, current, worker) = fixture(
            "artifacts/x86_64-pc-windows-msvc/release",
            "build/sai/hash/out/sai-hash.exe",
        );
        assert_eq!(resolve(&current).unwrap(), worker);
    }

    /// 【Windows命令】【缺失启动器】验证缺失时报告目标路径，不使用相邻或祖先的同名程序。
    /// 参数：无；返回：无。
    #[test]
    fn missing_worker_does_not_use_other_executables() {
        let (root, current, worker) = fixture("target/debug", "deps/sai-hash.exe");
        std::fs::remove_file(&worker).unwrap();
        std::fs::write(current.parent().unwrap().join("sai.exe"), b"decoy").unwrap();
        std::fs::write(root.path().join("sai.exe"), b"decoy").unwrap();

        let error = resolve(&current).unwrap_err().to_string();
        assert!(error.contains("build sai.exe before running Windows command tests"));
        assert!(error.contains(&worker.display().to_string()));
    }

    /// 【Windows命令】【文件校验】验证同名目录不能充当启动器。
    /// 参数：无；返回：无。
    #[test]
    fn rejects_directory_at_worker_path() {
        let (_root, current, worker) = fixture("target/debug", "deps/sai-hash.exe");
        std::fs::remove_file(&worker).unwrap();
        std::fs::create_dir(&worker).unwrap();
        assert!(resolve(&current).is_err());
    }

    /// 【Windows命令】【布局校验】验证未知目录布局不会接受相邻的同名程序。
    /// 参数：无；返回：无。
    #[test]
    fn rejects_unknown_layout() {
        let (_root, current, _worker) = fixture("target/debug", "other/sai-hash.exe");
        assert!(resolve(&current).is_err());
    }

    /// 【Windows命令】【绝对路径】验证相对路径不会从当前工作目录定位启动器。
    /// 参数：无；返回：无。
    #[test]
    fn rejects_relative_path() {
        assert!(resolve(Path::new("target/debug/deps/sai-hash.exe")).is_err());
    }
}
