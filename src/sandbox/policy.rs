use super::protected_paths::{
    deny_read_paths, expand_path, external_git_dirs, write_protected_paths,
};
use super::settings::SandboxSettings;
use crate::config::SandboxNetworkMode;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// 沙箱文件系统权限级别。
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FileAccess {
    /// 只允许写私有临时目录，供计划模式使用
    ReadOnly,
    /// 允许写工作区、Git 目录、额外可写目录与私有临时目录
    WorkspaceWrite,
}

impl FileAccess {
    /// 返回状态展示使用的稳定字符串。
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read_only",
            Self::WorkspaceWrite => "workspace_write",
        }
    }
}

/// 单次命令使用的已解析沙箱策略；所有路径均为规范化的绝对路径。
#[derive(Debug, Clone)]
pub(crate) struct SandboxPolicy {
    /// 文件系统权限级别
    pub access: FileAccess,
    /// 命令启动目录
    pub cwd: PathBuf,
    /// 可写目录
    pub writable_roots: Vec<PathBuf>,
    /// 可写目录中仍保持只读的子路径
    pub write_protected: Vec<PathBuf>,
    /// 沙箱内隐藏的路径
    pub deny_read: Vec<PathBuf>,
    /// 是否保留网络
    pub network: bool,
    /// 私有临时目录
    pub temp_dir: Option<PathBuf>,
    /// 命令最终使用的环境变量
    pub env: Vec<(OsString, OsString)>,
    /// 被移除的密钥变量名，用于状态展示
    pub scrubbed_env: Vec<String>,
}

impl SandboxPolicy {
    /// 【沙箱】【策略解析】按配置快照解析单次命令的沙箱策略。
    ///
    /// 参数:
    /// - `settings`: 沙箱配置快照
    /// - `access`: 文件系统权限级别
    /// - `workspace`: 工作区根目录
    /// - `cwd`: 命令工作目录；不在工作区内或不存在时回退工作区
    ///
    /// 返回:
    /// - 已解析策略
    pub(crate) fn resolve(
        settings: &SandboxSettings,
        access: FileAccess,
        workspace: &Path,
        cwd: Option<&Path>,
    ) -> Self {
        let workspace = canonical(workspace);
        let cwd = cwd
            .map(canonical)
            .filter(|dir| dir.is_dir() && dir.starts_with(&workspace))
            .unwrap_or_else(|| workspace.clone());
        let temp_dir = super::temp_dir::session_temp_dir();
        // 1. 可写目录：只读模式只留临时目录
        let mut writable_roots = Vec::new();
        if access == FileAccess::WorkspaceWrite {
            writable_roots.push(workspace.clone());
            writable_roots.extend(external_git_dirs(&workspace));
            writable_roots.extend(
                settings
                    .config
                    .writable_roots
                    .iter()
                    .filter_map(|item| expand_path(item, &workspace))
                    .map(|path| canonical(&path))
                    .filter(|path| path.exists()),
            );
        }
        let write_protected = write_protected_paths(&writable_roots);
        writable_roots.extend(temp_dir.clone());
        // 2. 隐藏路径不得覆盖可写目录或其祖先，否则工作区本身会消失
        let deny_read = deny_read_paths(
            &settings.config.deny_read,
            &settings.protected_files,
            &workspace,
        )
        .into_iter()
        .map(|path| canonical(&path))
        .filter(|path| {
            !writable_roots
                .iter()
                .any(|root| root.starts_with(path) || root == path)
                && !workspace.starts_with(path)
        })
        .collect();
        // 3. 环境变量：清理密钥，并把 TMPDIR 指向私有临时目录
        let (mut env, scrubbed_env) = super::env_scrub::scrub_env(
            std::env::vars_os(),
            settings.config.scrub_env,
            &settings.config.env_passthrough,
        );
        if let Some(dir) = &temp_dir {
            env.retain(|(key, _)| key != "TMPDIR");
            env.push((OsString::from("TMPDIR"), dir.clone().into_os_string()));
        }
        env.push((
            OsString::from("SAI_SANDBOX"),
            OsString::from(access.as_str()),
        ));
        Self {
            access,
            cwd,
            writable_roots: dedupe(writable_roots),
            write_protected,
            deny_read,
            network: settings.config.network == SandboxNetworkMode::Allow,
            temp_dir,
            env,
            scrubbed_env,
        }
    }

    /// 按全局配置快照与当前运行目录解析策略。
    ///
    /// 参数:
    /// - `access`: 文件系统权限级别
    /// - `cwd`: 可选命令工作目录
    ///
    /// 返回:
    /// - 已解析策略；无法读取当前目录时返回错误
    pub(crate) fn for_current_workspace(
        access: FileAccess,
        cwd: Option<&Path>,
    ) -> std::io::Result<Self> {
        let workspace = crate::runtime_cwd::current_dir()?;
        Ok(Self::resolve(
            &super::settings::current(),
            access,
            &workspace,
            cwd,
        ))
    }
}

/// 规范化路径；不存在时保留原值。
fn canonical(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
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
    use crate::config::SandboxConfig;

    /// 构造指定配置的快照。
    fn settings(config: SandboxConfig) -> SandboxSettings {
        SandboxSettings {
            config,
            protected_files: Vec::new(),
        }
    }

    /// 验证工作区写入策略包含工作区与临时目录，并保护 Git 钩子。
    #[test]
    fn workspace_write_policy_lists_roots() {
        let root = tempfile::tempdir().unwrap();
        let workspace = root.path().canonicalize().unwrap();
        let sub = workspace.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        let policy = SandboxPolicy::resolve(
            &settings(SandboxConfig::default()),
            FileAccess::WorkspaceWrite,
            &workspace,
            Some(&sub),
        );
        assert_eq!(policy.cwd, sub);
        assert_eq!(policy.writable_roots[0], workspace);
        assert!(policy
            .write_protected
            .contains(&workspace.join(".git/hooks")));
        assert!(!policy.network);
        assert!(policy.env.iter().any(|(key, _)| key == "SAI_SANDBOX"));
        if let Some(temp) = &policy.temp_dir {
            assert!(policy.writable_roots.contains(temp));
            assert!(policy
                .env
                .iter()
                .any(|(key, value)| key == "TMPDIR" && Path::new(value) == temp));
        }
    }

    /// 验证只读策略不包含工作区，越界 cwd 回退工作区。
    #[test]
    fn read_only_policy_excludes_workspace() {
        let root = tempfile::tempdir().unwrap();
        let workspace = root.path().canonicalize().unwrap();
        let policy = SandboxPolicy::resolve(
            &settings(SandboxConfig::default()),
            FileAccess::ReadOnly,
            &workspace,
            Some(Path::new("/")),
        );
        assert_eq!(policy.cwd, workspace);
        assert!(!policy.writable_roots.contains(&workspace));
    }

    /// 验证隐藏路径不会遮住工作区，网络与额外可写目录按配置生效。
    #[test]
    fn deny_read_never_hides_workspace() {
        let root = tempfile::tempdir().unwrap();
        let workspace = root.path().canonicalize().unwrap();
        let cache = workspace.join("cache");
        std::fs::create_dir_all(&cache).unwrap();
        let config = SandboxConfig {
            network: SandboxNetworkMode::Allow,
            writable_roots: vec![cache.display().to_string(), "/does/not/exist".into()],
            deny_read: vec![workspace.display().to_string(), ".env".into()],
            ..SandboxConfig::default()
        };
        let policy = SandboxPolicy::resolve(
            &settings(config),
            FileAccess::WorkspaceWrite,
            &workspace,
            None,
        );
        assert!(policy.network);
        assert!(policy.writable_roots.contains(&cache));
        assert!(!policy.writable_roots.iter().any(|p| p.starts_with("/does")));
        assert!(!policy.deny_read.contains(&workspace));
        assert!(policy.deny_read.contains(&workspace.join(".env")));
    }
}
