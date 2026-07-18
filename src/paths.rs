use crate::i18n::text as t;
use anyhow::{Context, Result};
use directories::{BaseDirs, UserDirs};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct SaiPaths {
    pub config_dir: PathBuf,
    pub config_file: PathBuf,
    pub secrets_file: PathBuf,
    pub skills_dir: PathBuf,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub state_dir: PathBuf,
    pub pictures_dir: PathBuf,
    pub fish_hook_file: PathBuf,
    pub bash_hook_file: PathBuf,
    pub zsh_hook_file: PathBuf,
    pub powershell_hook_file: PathBuf,
}

impl SaiPaths {
    pub fn new() -> Result<Self> {
        let base = BaseDirs::new().context(t(
            "could not determine platform base directories",
            "无法确定系统基础目录",
        ))?;
        let config_dir = base.config_dir().join("sai");
        let data_dir = base.data_dir().join("sai");
        let cache_dir = base.cache_dir().join("sai");
        let state_dir = state_base_dir(&base).join("sai");
        let pictures_dir = std::env::var_os("XDG_PICTURES_DIR")
            .map(PathBuf::from)
            .or_else(|| UserDirs::new().and_then(|dirs| dirs.picture_dir().map(PathBuf::from)))
            .unwrap_or_else(|| base.home_dir().join("Pictures"))
            .join("sai");
        let fish_hook_file = fish_hook_path(&base);
        let bash_hook_file = config_dir.join("shell/bash-hook.sh");
        let zsh_hook_file = config_dir.join("shell/zsh-hook.zsh");
        let powershell_hook_file = config_dir.join("shell/powershell-hook.ps1");

        Ok(Self {
            config_file: config_dir.join("config.jsonc"),
            secrets_file: config_dir.join("secrets.jsonc"),
            skills_dir: config_dir.join("skills"),
            config_dir,
            data_dir,
            cache_dir,
            state_dir,
            pictures_dir,
            fish_hook_file,
            bash_hook_file,
            zsh_hook_file,
            powershell_hook_file,
        })
    }

    /// 返回 MCP 独立配置文件路径。
    pub fn mcp_config_file(&self) -> PathBuf {
        self.config_dir.join("mcp.jsonc")
    }

    pub fn create_dirs(&self) -> Result<()> {
        std::fs::create_dir_all(&self.config_dir)?;
        std::fs::create_dir_all(&self.skills_dir)?;
        std::fs::create_dir_all(&self.data_dir)?;
        std::fs::create_dir_all(&self.cache_dir)?;
        std::fs::create_dir_all(&self.state_dir)?;
        std::fs::create_dir_all(&self.pictures_dir)?;
        Ok(())
    }

    pub fn print(&self) {
        println!(
            "{}: {}",
            t("config_dir", "配置目录"),
            self.config_dir.display()
        );
        println!(
            "{}: {}",
            t("config_file", "配置文件"),
            self.config_file.display()
        );
        println!(
            "{}: {}",
            t("secrets_file", "密钥文件"),
            self.secrets_file.display()
        );
        println!(
            "{}: {}",
            t("mcp_config_file", "MCP 配置文件"),
            self.mcp_config_file().display()
        );
        println!(
            "{}: {}",
            t("skills_dir", "skills 目录"),
            self.skills_dir.display()
        );
        println!("{}: {}", t("data_dir", "数据目录"), self.data_dir.display());
        println!(
            "{}: {}",
            t("cache_dir", "缓存目录"),
            self.cache_dir.display()
        );
        println!(
            "{}: {}",
            t("state_dir", "状态目录"),
            self.state_dir.display()
        );
        println!(
            "{}: {}",
            t("pictures_dir", "图片目录"),
            self.pictures_dir.display()
        );
        println!(
            "{}: {}",
            t("fish_hook_file", "fish hook 文件"),
            self.fish_hook_file.display()
        );
        println!(
            "{}: {}",
            t("bash_hook_file", "bash hook 文件"),
            self.bash_hook_file.display()
        );
        println!(
            "{}: {}",
            t("zsh_hook_file", "zsh hook 文件"),
            self.zsh_hook_file.display()
        );
        println!(
            "{}: {}",
            t("powershell_hook_file", "PowerShell hook 文件"),
            self.powershell_hook_file.display()
        );
    }
}

/// 返回当前平台适合持久化会话状态的目录。
///
/// 参数:
/// - `base`: `directories` 提供的平台目录
///
/// 返回:
/// - Linux 的 XDG 状态目录、Windows 的本地应用数据目录，或 macOS 的应用支持目录
/// - Windows 检测到旧版漫游目录时继续使用旧目录，避免升级后丢失会话
fn state_base_dir(base: &BaseDirs) -> PathBuf {
    #[cfg(windows)]
    {
        let local = base.data_local_dir().to_path_buf();
        let legacy = base.data_dir().join("sai");
        if legacy.exists() && !local.join("sai").exists() {
            return base.data_dir().to_path_buf();
        }
        return local;
    }
    #[cfg(not(windows))]
    {
        base.state_dir()
            .map(PathBuf::from)
            .unwrap_or_else(|| base.data_dir().to_path_buf())
    }
}

/// 返回 fish 自动加载的 hook 文件路径。
///
/// 参数:
/// - `base`: `directories` 提供的平台目录
///
/// 返回:
/// - fish 的 `conf.d` hook 文件路径
fn fish_hook_path(base: &BaseDirs) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        let config_dir = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| base.home_dir().join(".config"));
        return config_dir.join("fish/conf.d/sai.fish");
    }
    #[cfg(not(target_os = "macos"))]
    {
        base.config_dir().join("fish/conf.d/sai.fish")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fish_hook_path_uses_a_fish_config_directory() {
        let base = BaseDirs::new().expect("home directory should be available");
        let path = fish_hook_path(&base);
        assert!(path.ends_with("fish/conf.d/sai.fish"));
        #[cfg(target_os = "macos")]
        {
            let config_dir = std::env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| base.home_dir().join(".config"));
            assert!(path.starts_with(config_dir));
        }
        #[cfg(not(target_os = "macos"))]
        assert!(path.starts_with(base.config_dir()));
    }
}
