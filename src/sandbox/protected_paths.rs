use std::path::{Path, PathBuf};

/// 沙箱内默认隐藏的家目录凭据位置。
const HOME_CREDENTIAL_PATHS: &[&str] = &[
    ".ssh",
    ".gnupg",
    ".aws",
    ".azure",
    ".kube",
    ".docker/config.json",
    ".config/gcloud",
    ".config/gh/hosts.yml",
    ".netrc",
    ".git-credentials",
    ".npmrc",
    ".pypirc",
    ".cargo/credentials",
    ".cargo/credentials.toml",
];

/// 可写目录内仍保持只读的子路径：防止命令植入 Git 钩子借下一次提交逃出沙箱。
const WRITE_PROTECTED_CHILDREN: &[&str] = &[".git/hooks"];

/// 返回当前用户家目录。
///
/// 返回:
/// - `HOME`（Windows 为 `USERPROFILE`）指向的目录
pub(crate) fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// 展开 `~/` 前缀并把相对路径按基准目录解析。
///
/// 参数:
/// - `value`: 配置中的路径
/// - `base`: 相对路径的基准目录
///
/// 返回:
/// - 绝对路径；空值或无法展开时为 `None`
pub(crate) fn expand_path(value: &str, base: &Path) -> Option<PathBuf> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if value == "~" {
        return home_dir();
    }
    if let Some(rest) = value.strip_prefix("~/") {
        return home_dir().map(|home| home.join(rest));
    }
    let path = PathBuf::from(value);
    Some(if path.is_absolute() {
        path
    } else {
        base.join(path)
    })
}

/// 返回内置隐藏路径与用户追加路径的合集。
///
/// 参数:
/// - `extra`: 配置 `sandbox.deny_read`
/// - `protected_files`: sai 自身凭据文件
/// - `workspace`: 相对路径基准
///
/// 返回:
/// - 去重后的绝对路径列表
pub(crate) fn deny_read_paths(
    extra: &[String],
    protected_files: &[PathBuf],
    workspace: &Path,
) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    // 1. 家目录凭据
    if let Some(home) = home_dir() {
        paths.extend(HOME_CREDENTIAL_PATHS.iter().map(|item| home.join(item)));
    }
    // 2. sai 配置、密钥与 MCP 文件
    paths.extend(protected_files.iter().cloned());
    // 3. 用户追加项
    paths.extend(extra.iter().filter_map(|item| expand_path(item, workspace)));
    dedupe(paths)
}

/// 返回可写根目录下需要保持只读的子路径。
///
/// 参数:
/// - `roots`: 可写根目录
///
/// 返回:
/// - 只读子路径，包含尚不存在的路径（Seatbelt 可预先拦截）
pub(crate) fn write_protected_paths(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for root in roots {
        paths.extend(
            WRITE_PROTECTED_CHILDREN
                .iter()
                .map(|child| root.join(child)),
        );
        // 工作树与子目录工作区的真实 Git 目录单独登记为可写根，钩子目录在其下
        if root.file_name().is_some_and(|name| name == ".git") || root.join("HEAD").is_file() {
            paths.push(root.join("hooks"));
        }
    }
    dedupe(paths)
}

/// 定位工作区使用的 Git 目录，处理子目录工作区与 `git worktree`。
///
/// 参数:
/// - `workspace`: 工作区根目录
///
/// 返回:
/// - 工作区外、需要额外可写的 Git 目录（worktree 私有目录与公共目录）
pub(crate) fn external_git_dirs(workspace: &Path) -> Vec<PathBuf> {
    // 1. 自工作区向上寻找 `.git`
    let Some(dot_git) = workspace
        .ancestors()
        .map(|dir| dir.join(".git"))
        .find(|candidate| candidate.exists())
    else {
        return Vec::new();
    };
    let mut dirs = Vec::new();
    // 2. `.git` 为文件时读取 gitdir 指向的 worktree 目录及其 commondir
    let git_dir = if dot_git.is_file() {
        let Some(target) = read_gitdir_file(&dot_git) else {
            return Vec::new();
        };
        target
    } else {
        dot_git
    };
    if let Some(common) = read_commondir(&git_dir) {
        dirs.push(common);
    }
    dirs.push(git_dir);
    dirs.into_iter()
        .map(|dir| dir.canonicalize().unwrap_or(dir))
        .filter(|dir| !dir.starts_with(workspace))
        .collect()
}

/// 解析 `.git` 文件中的 `gitdir:` 行。
fn read_gitdir_file(dot_git: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(dot_git).ok()?;
    let value = text.lines().find_map(|line| line.strip_prefix("gitdir:"))?;
    let path = PathBuf::from(value.trim());
    let base = dot_git.parent()?;
    Some(if path.is_absolute() {
        path
    } else {
        base.join(path)
    })
}

/// 解析 worktree 目录中的 `commondir` 文件。
fn read_commondir(git_dir: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(git_dir.join("commondir")).ok()?;
    let path = PathBuf::from(text.trim());
    Some(if path.is_absolute() {
        path
    } else {
        git_dir.join(path)
    })
}

/// 保序去重。
fn dedupe(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = std::collections::HashSet::new();
    paths
        .into_iter()
        .filter(|path| seen.insert(path.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证相对路径按工作区解析、`~/` 按家目录展开。
    #[test]
    fn expands_relative_and_home_paths() {
        let base = Path::new("/work");
        assert_eq!(expand_path(".env", base), Some(PathBuf::from("/work/.env")));
        assert_eq!(expand_path("/etc/x", base), Some(PathBuf::from("/etc/x")));
        assert_eq!(expand_path("  ", base), None);
        if let Some(home) = home_dir() {
            assert_eq!(expand_path("~/.cache", base), Some(home.join(".cache")));
        }
    }

    /// 验证默认隐藏列表包含 SSH 目录与 sai 密钥文件。
    #[test]
    fn deny_list_includes_credentials_and_sai_secrets() {
        let secrets = PathBuf::from("/cfg/secrets.json");
        let paths = deny_read_paths(
            &[".env".into()],
            std::slice::from_ref(&secrets),
            Path::new("/w"),
        );
        assert!(paths.contains(&secrets));
        assert!(paths.contains(&PathBuf::from("/w/.env")));
        if let Some(home) = home_dir() {
            assert!(paths.contains(&home.join(".ssh")));
        }
    }

    /// 验证 worktree 的私有目录与公共目录都登记为额外可写目录。
    #[test]
    fn resolves_worktree_git_dirs() {
        let root = tempfile::tempdir().unwrap();
        let root_path = root.path().canonicalize().unwrap();
        let common = root_path.join("main/.git");
        let private = common.join("worktrees/feature");
        std::fs::create_dir_all(&private).unwrap();
        std::fs::write(private.join("commondir"), "../..").unwrap();
        let workspace = root_path.join("feature");
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::write(
            workspace.join(".git"),
            format!("gitdir: {}\n", private.display()),
        )
        .unwrap();

        let dirs = external_git_dirs(&workspace);
        assert!(dirs.contains(&common));
        assert!(dirs.contains(&private));
        let protected = write_protected_paths(&dirs);
        assert!(protected.contains(&common.join("hooks")));
    }

    /// 验证普通仓库的 `.git` 位于工作区内时不重复登记。
    #[test]
    fn workspace_git_dir_needs_no_extra_root() {
        let root = tempfile::tempdir().unwrap();
        let workspace = root.path().canonicalize().unwrap();
        std::fs::create_dir_all(workspace.join(".git")).unwrap();
        assert!(external_git_dirs(&workspace).is_empty());
        assert!(write_protected_paths(std::slice::from_ref(&workspace))
            .contains(&workspace.join(".git/hooks")));
    }
}
