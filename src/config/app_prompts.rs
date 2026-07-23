use super::model::AppConfig;
use super::paths::{config_relative_path, persona_scope_name};
use crate::paths::SaiPaths;
use crate::prompts::default_system_prompt;
use anyhow::{Context, Result};
use std::path::PathBuf;

impl AppConfig {
    /// 组装运行时使用的完整系统提示词。
    ///
    /// 参数:
    /// - `paths`: 应用目录集合
    ///
    /// 返回:
    /// - 合并当前用户身份信息后的系统提示词
    pub fn system_prompt(&self, paths: &SaiPaths) -> Result<String> {
        let mut prompt = self.base_system_prompt(paths)?;
        let user_identity = self.user_identity_prompt(paths)?;
        if !user_identity.trim().is_empty() {
            prompt.push_str("\n\n<current-user-profile>\n");
            prompt.push_str("This profile describes the user currently interacting with you.\n\n");
            prompt.push_str(user_identity.trim());
            prompt.push_str("\n</current-user-profile>");
        }
        Ok(prompt)
    }

    /// 解析不含用户身份信息的基础系统提示词。
    ///
    /// 参数:
    /// - `paths`: 应用目录集合
    ///
    /// 返回:
    /// - Agent 覆盖、旧文件或内置默认提示词
    pub fn base_system_prompt(&self, paths: &SaiPaths) -> Result<String> {
        // 1. Agent 档案 / 运行时覆盖写入的 system_prompt 优先
        if let Some(prompt) = self
            .system_prompt
            .as_deref()
            .map(str::trim)
            .filter(|prompt| !prompt.is_empty())
        {
            return Ok(prompt.to_string());
        }
        // 2. 兼容旧 system-prompt.md 文件，否则使用内置默认人设提示
        let legacy = self.custom_system_prompt(paths)?;
        if !legacy.trim().is_empty() {
            return Ok(legacy);
        }
        Ok(default_system_prompt())
    }

    /// 读取配置指定的自定义系统提示词。
    ///
    /// 参数:
    /// - `paths`: 应用目录集合
    ///
    /// 返回:
    /// - 自定义提示词；未配置时返回空字符串
    pub fn custom_system_prompt(&self, paths: &SaiPaths) -> Result<String> {
        if let Some(prompt) = self
            .system_prompt
            .as_deref()
            .filter(|prompt| !prompt.trim().is_empty())
        {
            return Ok(prompt.to_string());
        }
        let prompt_file = self.system_prompt_path(paths);
        if prompt_file.exists() {
            return Ok(std::fs::read_to_string(prompt_file)?);
        }
        Ok(String::new())
    }

    /// 解析 persona 提示词目录。
    ///
    /// 参数:
    /// - `paths`: 应用目录集合
    ///
    /// 返回:
    /// - 绝对提示词目录路径
    pub fn prompts_dir_path(&self, paths: &SaiPaths) -> PathBuf {
        config_relative_path(paths, &self.prompt.prompts_dir)
    }

    /// 解析默认用户身份文件路径。
    ///
    /// 参数:
    /// - `paths`: 应用目录集合
    ///
    /// 返回:
    /// - 绝对用户身份文件路径
    pub fn user_identity_path(&self, paths: &SaiPaths) -> PathBuf {
        config_relative_path(paths, &self.prompt.user_identity_file)
    }

    /// 解析用户身份档案目录。
    ///
    /// 参数:
    /// - `paths`: 应用目录集合
    ///
    /// 返回:
    /// - 绝对身份档案目录路径
    pub fn identities_dir_path(&self, paths: &SaiPaths) -> PathBuf {
        config_relative_path(paths, &self.prompt.identities_dir)
    }

    /// 解析指定 persona 提示词路径。
    ///
    /// 参数:
    /// - `paths`: 应用目录集合
    /// - `name`: persona 文件名
    ///
    /// 返回:
    /// - persona 提示词路径
    pub fn persona_path(&self, paths: &SaiPaths, name: &str) -> PathBuf {
        self.prompts_dir_path(paths).join(name)
    }

    /// 解析指定身份档案路径。
    ///
    /// 参数:
    /// - `paths`: 应用目录集合
    /// - `name`: 身份档案文件名
    ///
    /// 返回:
    /// - 身份档案路径
    pub fn identity_path(&self, paths: &SaiPaths, name: &str) -> PathBuf {
        self.identities_dir_path(paths).join(name)
    }

    /// 解析指定 persona 的长期记忆数据目录。
    ///
    /// 参数:
    /// - `paths`: 应用目录集合
    /// - `persona`: persona 名称
    ///
    /// 返回:
    /// - persona 长期记忆数据目录
    pub fn persona_memory_data_dir(&self, paths: &SaiPaths, persona: &str) -> PathBuf {
        paths
            .data_dir
            .join("personas")
            .join(persona_scope_name(persona))
    }

    /// 解析指定 persona 的运行状态目录。
    ///
    /// 参数:
    /// - `paths`: 应用目录集合
    /// - `persona`: persona 名称
    ///
    /// 返回:
    /// - persona 运行状态目录
    pub fn persona_memory_state_dir(&self, paths: &SaiPaths, persona: &str) -> PathBuf {
        paths
            .state_dir
            .join("personas")
            .join(persona_scope_name(persona))
    }

    /// 解析指定 persona 的技能目录。
    ///
    /// 参数:
    /// - `paths`: 应用目录集合
    /// - `persona`: persona 名称
    ///
    /// 返回:
    /// - persona 技能目录
    pub fn persona_skills_dir(&self, paths: &SaiPaths, persona: &str) -> PathBuf {
        paths
            .skills_dir
            .join("personas")
            .join(persona_scope_name(persona))
    }

    /// 解析当前 persona 的长期记忆数据目录。
    ///
    /// 参数:
    /// - `paths`: 应用目录集合
    ///
    /// 返回:
    /// - 当前 persona 长期记忆数据目录
    pub fn active_persona_memory_data_dir(&self, paths: &SaiPaths) -> PathBuf {
        self.persona_memory_data_dir(paths, self.prompt.active_persona.trim())
    }

    /// 解析当前 persona 的运行状态目录。
    ///
    /// 参数:
    /// - `paths`: 应用目录集合
    ///
    /// 返回:
    /// - 当前 persona 运行状态目录
    pub fn active_persona_memory_state_dir(&self, paths: &SaiPaths) -> PathBuf {
        self.persona_memory_state_dir(paths, self.prompt.active_persona.trim())
    }

    /// 解析当前 persona 的技能目录。
    ///
    /// 参数:
    /// - `paths`: 应用目录集合
    ///
    /// 返回:
    /// - 当前 persona 技能目录
    pub fn active_persona_skills_dir(&self, paths: &SaiPaths) -> PathBuf {
        self.persona_skills_dir(paths, self.prompt.active_persona.trim())
    }

    /// 读取当前激活的用户身份提示词。
    ///
    /// 参数:
    /// - `paths`: 应用目录集合
    ///
    /// 返回:
    /// - 身份提示词；文件不存在时返回空字符串
    pub fn user_identity_prompt(&self, paths: &SaiPaths) -> Result<String> {
        if !self.prompt.active_identity.trim().is_empty() {
            let path = self.identity_path(paths, self.prompt.active_identity.trim());
            if path.exists() {
                return std::fs::read_to_string(&path)
                    .with_context(|| format!("failed to read {}", path.display()));
            }
        }
        let path = self.user_identity_path(paths);
        if path.exists() {
            return std::fs::read_to_string(&path)
                .with_context(|| format!("failed to read {}", path.display()));
        }
        Ok(String::new())
    }

    /// 解析自定义系统提示词文件路径。
    ///
    /// 参数:
    /// - `paths`: 应用目录集合
    ///
    /// 返回:
    /// - 绝对系统提示词文件路径
    pub fn system_prompt_path(&self, paths: &SaiPaths) -> PathBuf {
        let value = self
            .system_prompt_file
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("system-prompt.md");
        let path = PathBuf::from(value);
        if path.is_absolute() {
            path
        } else {
            paths.config_dir.join(path)
        }
    }
}
