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
            "could not determine XDG base directories",
            "无法确定 XDG 基础目录",
        ))?;
        let config_dir = base.config_dir().join("sai");
        let data_dir = base.data_dir().join("sai");
        let cache_dir = base.cache_dir().join("sai");
        let state_dir = base
            .state_dir()
            .unwrap_or_else(|| base.data_dir())
            .join("sai");
        let pictures_dir = std::env::var_os("XDG_PICTURES_DIR")
            .map(PathBuf::from)
            .or_else(|| UserDirs::new().and_then(|dirs| dirs.picture_dir().map(PathBuf::from)))
            .unwrap_or_else(|| base.home_dir().join("Pictures"))
            .join("sai");
        let fish_hook_file = base.config_dir().join("fish/conf.d/sai.fish");
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
