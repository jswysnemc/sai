use super::app_validation::{
    default_context_chars_for_provider_model, validate_provider_model_metadata,
};
use super::model::*;
use super::paths::{config_relative_path, persona_scope_name};
use super::secrets::set_private_permissions;
use crate::default_models::OPENCODE_PROVIDER_ID;
use crate::paths::SaiPaths;
use crate::prompts::default_system_prompt;
use anyhow::{bail, Context, Result};
use std::collections::HashSet;
use std::net::SocketAddr;
use std::path::PathBuf;

impl AppConfig {
    pub fn memory_config(&self) -> &MemoryConfig {
        if self.memory != MemoryConfig::default() {
            &self.memory
        } else {
            &self.plugins.memory
        }
    }

    pub fn load(paths: &SaiPaths) -> Result<Self> {
        let raw = std::fs::read_to_string(&paths.config_file)
            .with_context(|| format!("failed to read {}", paths.config_file.display()))?;
        let stripped = json_comments::StripComments::new(raw.as_bytes());
        let mut config: Self = serde_json::from_reader(stripped)
            .with_context(|| format!("invalid JSONC in {}", paths.config_file.display()))?;
        config.normalize_builtin_providers();
        config.validate()?;
        Ok(config)
    }

    pub fn load_or_default(paths: &SaiPaths) -> Result<Self> {
        if paths.config_file.exists() {
            Self::load(paths)
        } else {
            Ok(Self::default())
        }
    }

    pub fn init_files(paths: &SaiPaths) -> Result<()> {
        paths.create_dirs()?;
        if !paths.config_file.exists() {
            Self::default().save(paths)?;
        }
        if !paths.secrets_file.exists() {
            let raw = "{\n  // Optional provider API keys. Prefer $env:... in config.jsonc.\n  \"api_keys\": {}\n}\n";
            std::fs::write(&paths.secrets_file, raw)?;
            set_private_permissions(&paths.secrets_file)?;
        }
        Ok(())
    }

    pub fn save(&self, paths: &SaiPaths) -> Result<()> {
        paths.create_dirs()?;
        let mut config = self.clone();
        if let Some(prompt) = config.system_prompt.take() {
            let prompt_file = config.system_prompt_path(paths);
            if let Some(parent) = prompt_file.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let prompt = prompt.trim_end();
            let content = if prompt.is_empty() {
                String::new()
            } else {
                format!("{prompt}\n")
            };
            std::fs::write(prompt_file, content)?;
        }
        if config
            .system_prompt_file
            .as_deref()
            .map(str::trim)
            .unwrap_or_default()
            .is_empty()
        {
            config.system_prompt_file = Some("system-prompt.md".to_string());
        }
        let raw = serde_json::to_string_pretty(&config)?;
        std::fs::write(&paths.config_file, format!("{raw}\n"))?;
        Ok(())
    }

    fn normalize_builtin_providers(&mut self) {
        for provider in ProviderConfig::default_templates() {
            if !self.providers.iter().any(|item| {
                item.id == provider.id
                    || provider.id == OPENCODE_PROVIDER_ID && item.is_opencode_zen()
            }) {
                self.providers.push(provider);
            }
        }
        if self.active_provider == "opencodezen" {
            self.active_provider = OPENCODE_PROVIDER_ID.to_string();
        }
        if self.plugins.vision.vision_provider_id == "opencodezen" {
            self.plugins.vision.vision_provider_id = OPENCODE_PROVIDER_ID.to_string();
        }
        if self
            .provider(None)
            .map(|provider| provider.default_model.trim().is_empty())
            .unwrap_or(true)
        {
            self.active_provider = OPENCODE_PROVIDER_ID.to_string();
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.active_provider.trim().is_empty() {
            bail!("active_provider cannot be empty");
        }
        if self.providers.is_empty() {
            bail!("at least one provider is required");
        }
        for provider in &self.providers {
            if provider.id.trim().is_empty() {
                bail!("provider id cannot be empty");
            }
            if provider.base_url.trim().is_empty() {
                bail!("provider {} base_url cannot be empty", provider.id);
            }
            if provider.timeout_seconds == 0 {
                bail!(
                    "provider {} timeout_seconds must be greater than 0",
                    provider.id
                );
            }
            if !(0.0..=2.0).contains(&provider.temperature) {
                bail!(
                    "provider {} temperature must be between 0.0 and 2.0",
                    provider.id
                );
            }
            if provider.anthropic_max_tokens == 0 {
                bail!(
                    "provider {} anthropic_max_tokens must be greater than 0",
                    provider.id
                );
            }
            match provider.thinking_level.trim() {
                "" | "auto" | "none" | "low" | "medium" | "high" | "xhigh" | "max" => {}
                value => bail!(
                    "provider {} thinking_level is invalid: {value}",
                    provider.id
                ),
            }
            match provider.thinking_format.trim() {
                ""
                | "auto"
                | "string"
                | "object"
                | "deepseek-thinking"
                | "openai-chat-reasoning-effort"
                | "reasoning"
                | "anthropic-thinking"
                | "disabled" => {}
                value => bail!(
                    "provider {} thinking_format is invalid: {value}",
                    provider.id
                ),
            }
            if !provider.extra_body.trim().is_empty() {
                let extra_body = serde_json::from_str::<serde_json::Value>(&provider.extra_body)
                    .with_context(|| {
                        format!("provider {} extra_body is invalid JSON", provider.id)
                    })?;
                if !extra_body.is_object() {
                    bail!("provider {} extra_body must be a JSON object", provider.id);
                }
            }
            validate_provider_model_metadata(provider)?;
        }
        let mut agent_ids = HashSet::new();
        for agent in &self.agents {
            if agent.id.trim().is_empty() {
                bail!("agent id cannot be empty");
            }
            if !agent_ids.insert(agent.id.as_str()) {
                bail!("duplicate agent id: {}", agent.id);
            }
            match agent.thinking_level.trim() {
                "" | "auto" | "none" | "low" | "medium" | "high" | "xhigh" | "max" => {}
                value => bail!("agent {} thinking_level is invalid: {value}", agent.id),
            }
            if !agent.provider_id.trim().is_empty()
                && !self
                    .providers
                    .iter()
                    .any(|provider| provider.id == agent.provider_id)
            {
                bail!(
                    "agent {} provider not found: {}",
                    agent.id,
                    agent.provider_id
                );
            }
        }
        let resolved_agent_ids = self
            .resolved_agent_profiles()
            .into_iter()
            .map(|profile| profile.id)
            .collect::<HashSet<_>>();
        for (surface, selected) in [
            ("web", self.default_agent.as_deref()),
            ("tui", self.tui_agent.as_deref()),
            ("cli", self.cli_agent.as_deref()),
            ("gateway", self.gateway_agent.as_deref()),
        ] {
            let Some(selected) = selected.map(str::trim).filter(|value| !value.is_empty()) else {
                continue;
            };
            if selected != super::DEFAULT_AGENT_ID && !resolved_agent_ids.contains(selected) {
                bail!("{surface} default agent not found: {selected}");
            }
        }
        if self.context.default_max_chars == 0 {
            bail!("context.default_max_chars must be greater than 0");
        }
        if self.tools.background_command_log_max_bytes == 0 {
            bail!("tools.background_command_log_max_bytes must be greater than 0");
        }
        if self.tools.background_command_stop_grace_seconds == 0 {
            bail!("tools.background_command_stop_grace_seconds must be greater than 0");
        }
        self.validate_gateways()?;
        self.validate_compaction_model()?;
        if self.plugins.print_image.width_percent == 0
            || self.plugins.print_image.width_percent > 100
        {
            bail!("plugins.print_image.width_percent must be between 1 and 100");
        }
        if self.plugins.print_image.height_percent == 0
            || self.plugins.print_image.height_percent > 100
        {
            bail!("plugins.print_image.height_percent must be between 1 and 100");
        }
        match self.plugins.deep_research.thinking_depth.as_str() {
            "minimal" | "low" | "medium" | "high" | "xhigh" => {}
            value => bail!("plugins.deep_research.thinking_depth is invalid: {value}"),
        }
        match self.plugins.deep_diagnose.thinking_depth.as_str() {
            "minimal" | "low" | "medium" | "high" | "xhigh" => {}
            value => bail!("plugins.deep_diagnose.thinking_depth is invalid: {value}"),
        }
        if self.plugins.deep_diagnose.tool_call_timeout_seconds == 0 {
            bail!("plugins.deep_diagnose.tool_call_timeout_seconds must be greater than 0");
        }
        match self.plugins.image_generation.provider_type.as_str() {
            "openai" | "rightcode" => {}
            value => bail!("plugins.image_generation.provider_type is invalid: {value}"),
        }
        match self.plugins.image_generation.default_aspect_ratio.as_str() {
            "自动" | "1:1" | "2:3" | "3:2" | "3:4" | "4:3" | "4:5" | "5:4" | "9:16" | "16:9"
            | "21:9" => {}
            value => bail!("plugins.image_generation.default_aspect_ratio is invalid: {value}"),
        }
        match self.plugins.image_generation.default_resolution.as_str() {
            "1K" | "2K" | "4K" => {}
            value => bail!("plugins.image_generation.default_resolution is invalid: {value}"),
        }
        if self.plugins.image_generation.timeout_seconds == 0 {
            bail!("plugins.image_generation.timeout_seconds must be greater than 0");
        }
        if self.plugins.knowledge_base.max_search_results == 0 {
            bail!("plugins.knowledge_base.max_search_results must be greater than 0");
        }
        if self.plugins.knowledge_base.max_read_lines == 0 {
            bail!("plugins.knowledge_base.max_read_lines must be greater than 0");
        }
        if self.plugins.knowledge_base.max_file_size_kb == 0 {
            bail!("plugins.knowledge_base.max_file_size_kb must be greater than 0");
        }
        if self.plugins.knowledge_base.semantic_chunk_chars < 128 {
            bail!("plugins.knowledge_base.semantic_chunk_chars must be at least 128");
        }
        if self.plugins.knowledge_base.semantic_chunk_overlap
            >= self.plugins.knowledge_base.semantic_chunk_chars
        {
            bail!("plugins.knowledge_base.semantic_chunk_overlap must be smaller than semantic_chunk_chars");
        }
        if self.plugins.knowledge_base.semantic_top_k == 0 {
            bail!("plugins.knowledge_base.semantic_top_k must be greater than 0");
        }
        if self.plugins.knowledge_base.embedding_timeout_seconds == 0 {
            bail!("plugins.knowledge_base.embedding_timeout_seconds must be greater than 0");
        }
        self.provider(None)?;
        Ok(())
    }

    /// 校验渠道接入配置。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 配置是否合法
    fn validate_gateways(&self) -> Result<()> {
        match self
            .gateways
            .qq
            .transport
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "" | "websocket" | "ws" | "webhook" | "http" => {}
            value => bail!("gateways.qq.transport is invalid: {value}"),
        }
        if !self.gateways.qq.listen.trim().is_empty() {
            self.gateways
                .qq
                .listen
                .parse::<SocketAddr>()
                .with_context(|| "gateways.qq.listen is invalid")?;
        }
        if self.gateways.qq.base_url.trim().is_empty() {
            bail!("gateways.qq.base_url cannot be empty");
        }
        if let Some((app_id, client_secret)) = self.gateways.qq.token.trim().split_once(':') {
            if app_id.trim().is_empty() || client_secret.trim().is_empty() {
                bail!("gateways.qq.token must use AppID:AppSecret format");
            }
        } else if !self.gateways.qq.token.trim().is_empty() {
            bail!("gateways.qq.token must use AppID:AppSecret format");
        }
        if self.gateways.weixin.base_url.trim().is_empty() {
            bail!("gateways.weixin.base_url cannot be empty");
        }
        if self.gateways.weixin.cdn_base_url.trim().is_empty() {
            bail!("gateways.weixin.cdn_base_url cannot be empty");
        }
        if self.gateways.weixin.bot_type.trim().is_empty() {
            bail!("gateways.weixin.bot_type cannot be empty");
        }
        Ok(())
    }

    pub fn provider(&self, id: Option<&str>) -> Result<&ProviderConfig> {
        let target = id.unwrap_or(&self.active_provider);
        self.providers
            .iter()
            .find(|provider| provider.id == target)
            .with_context(|| format!("provider not found: {target}"))
    }

    pub fn provider_model_choices(&self) -> Vec<ProviderModelChoice> {
        self.providers
            .iter()
            .flat_map(|provider| {
                let models =
                    if provider.models.is_empty() && !provider.default_model.trim().is_empty() {
                        vec![provider.default_model.clone()]
                    } else {
                        provider.models.clone()
                    };
                models
                    .into_iter()
                    .filter(|model| !model.trim().is_empty())
                    .map(|model| ProviderModelChoice {
                        provider_id: provider.id.clone(),
                        provider_name: provider.display_name.clone(),
                        model,
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    pub fn set_active_provider_model(&mut self, provider_id: &str, model: &str) -> Result<()> {
        let provider = self
            .providers
            .iter_mut()
            .find(|provider| provider.id == provider_id)
            .with_context(|| format!("provider not found: {provider_id}"))?;
        if model.trim().is_empty() {
            bail!("model cannot be empty");
        }
        self.active_provider = provider.id.clone();
        provider.default_model = model.to_string();
        if !provider.models.iter().any(|item| item == model) {
            provider.models.push(model.to_string());
        }
        Ok(())
    }

    /// 构造压缩请求使用的运行时配置。
    ///
    /// 未配置专用模型时原样返回当前配置，使压缩自动继承本轮会话模型覆盖。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 已应用压缩模型选择的独立配置副本
    pub fn compaction_runtime_config(&self) -> Result<Self> {
        self.validate_compaction_model()?;
        if self.context.compaction_provider_id.trim().is_empty() {
            return Ok(self.clone());
        }
        let mut config = self.clone();
        let provider_id = self.context.compaction_provider_id.trim();
        let model = self.context.compaction_model.trim();
        config.set_active_provider_model(provider_id, model)?;
        Ok(config)
    }

    /// 返回压缩请求实际使用的供应商与模型标签。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 供应商与模型选择
    pub fn compaction_provider_model(&self) -> Result<ProviderModelChoice> {
        let config = self.compaction_runtime_config()?;
        let provider = config.provider(None)?;
        if provider.default_model.trim().is_empty() {
            bail!("compaction model cannot be empty");
        }
        Ok(ProviderModelChoice {
            provider_id: provider.id.clone(),
            provider_name: provider.display_name.clone(),
            model: provider.default_model.clone(),
        })
    }

    /// 校验压缩供应商与模型必须同时配置。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 配置合法时成功
    fn validate_compaction_model(&self) -> Result<()> {
        let provider_id = self.context.compaction_provider_id.trim();
        let model = self.context.compaction_model.trim();
        match (provider_id.is_empty(), model.is_empty()) {
            (true, true) => Ok(()),
            (false, false) => {
                self.provider(Some(provider_id))?;
                Ok(())
            }
            _ => bail!(
                "context.compaction_provider_id and context.compaction_model must be provided together"
            ),
        }
    }

    /// 从指定 provider 的激活模型列表中移除模型，并清理相关元数据。
    ///
    /// 参数:
    /// - `provider_id`: provider 标识
    /// - `model`: 要移除的模型 ID
    ///
    /// 返回:
    /// - 移除是否成功
    pub fn remove_active_provider_model(&mut self, provider_id: &str, model: &str) -> Result<()> {
        let provider = self
            .providers
            .iter_mut()
            .find(|provider| provider.id == provider_id)
            .with_context(|| format!("provider not found: {provider_id}"))?;
        if model.trim().is_empty() {
            bail!("model cannot be empty");
        }
        // 1. 从激活列表移除
        provider.models.retain(|item| item != model);
        // 2. 清理上下文与元数据
        provider.model_context_chars.remove(model);
        provider.model_metadata.remove(model);
        // 3. 若移除的是当前默认模型，回退到列表首项
        if provider.default_model == model {
            provider.default_model = provider.models.first().cloned().unwrap_or_default();
        }
        if self.context.compaction_provider_id == provider_id
            && self.context.compaction_model == model
        {
            self.context.compaction_provider_id.clear();
            self.context.compaction_model.clear();
        }
        Ok(())
    }

    /// 删除供应商并清理所有关联模型引用。
    ///
    /// 参数:
    /// - `provider_id`: 要删除的供应商标识
    ///
    /// 返回:
    /// - 被删除的供应商配置；供应商不存在或仅剩一个供应商时返回错误
    pub fn remove_provider(&mut self, provider_id: &str) -> Result<ProviderConfig> {
        if self.providers.len() <= 1 {
            bail!("at least one provider is required");
        }
        let index = self
            .providers
            .iter()
            .position(|provider| provider.id == provider_id)
            .with_context(|| format!("provider not found: {provider_id}"))?;

        // 1. 删除供应商并为主对话选择有效回退项
        let removed = self.providers.remove(index);
        if self.active_provider == removed.id {
            self.active_provider = self
                .providers
                .first()
                .map(|provider| provider.id.clone())
                .unwrap_or_default();
        }

        // 2. 清理压缩、视觉、嵌入和子智能体的供应商及模型引用
        if self.context.compaction_provider_id == removed.id {
            self.context.compaction_provider_id.clear();
            self.context.compaction_model.clear();
        }
        if self.plugins.vision.vision_provider_id == removed.id {
            self.plugins.vision.vision_provider_id.clear();
            self.plugins.vision.vision_model.clear();
        }
        if self.plugins.knowledge_base.embedding_provider_id == removed.id {
            self.plugins.knowledge_base.embedding_provider_id.clear();
            self.plugins.knowledge_base.embedding_model.clear();
        }
        if self.subagent.provider_id == removed.id {
            self.subagent.provider_id.clear();
            self.subagent.model.clear();
        }

        Ok(removed)
    }

    /// 按模型标签选择当前 provider 和模型。
    ///
    /// 参数:
    /// - `tag`: 模型标签
    ///
    /// 返回:
    /// - 被选中的 provider/model
    pub fn select_active_provider_model_with_tag(
        &mut self,
        tag: &str,
    ) -> Result<ProviderModelChoice> {
        let tag = tag.trim();
        if tag.is_empty() {
            bail!("model tag cannot be empty");
        }
        let choices = self.provider_model_choices_with_tag(tag);
        let choice = choices
            .iter()
            .find(|choice| {
                self.active_provider == choice.provider_id
                    && self
                        .provider(Some(&choice.provider_id))
                        .map(|provider| provider.default_model == choice.model)
                        .unwrap_or(false)
            })
            .or_else(|| choices.first())
            .cloned()
            .with_context(|| format!("no active provider model has tag: {tag}"))?;
        self.set_active_provider_model(&choice.provider_id, &choice.model)?;
        Ok(choice)
    }

    /// 返回拥有指定标签的 provider/model 列表。
    ///
    /// 参数:
    /// - `tag`: 模型标签
    ///
    /// 返回:
    /// - 匹配的 provider/model 列表
    pub fn provider_model_choices_with_tag(&self, tag: &str) -> Vec<ProviderModelChoice> {
        let tag = tag.trim();
        self.providers
            .iter()
            .flat_map(|provider| {
                let models =
                    if provider.models.is_empty() && !provider.default_model.trim().is_empty() {
                        vec![provider.default_model.clone()]
                    } else {
                        provider.models.clone()
                    };
                models
                    .into_iter()
                    .filter(|model| {
                        !model.trim().is_empty()
                            && provider
                                .model_tags_for(model)
                                .iter()
                                .any(|item| item == tag)
                    })
                    .map(|model| ProviderModelChoice {
                        provider_id: provider.id.clone(),
                        provider_name: provider.display_name.clone(),
                        model,
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// 判断当前模型是否允许工具调用。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 未显式关闭时返回 true
    pub fn active_model_tools_enabled(&self) -> Result<bool> {
        let provider = self.provider(None)?;
        Ok(provider.model_tools_enabled_for(&provider.default_model))
    }

    /// 返回当前模型上下文窗口 token 数。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 当前模型上下文窗口 token 数
    pub fn active_context_window_tokens(&self) -> Result<usize> {
        let provider = self.provider(None)?;
        Ok(provider
            .model_context_chars_for(&provider.default_model)
            .or_else(|| default_context_chars_for_provider_model(provider, &provider.default_model))
            .unwrap_or(self.context.default_max_chars))
    }

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

    pub fn base_system_prompt(&self, paths: &SaiPaths) -> Result<String> {
        let persona = self.active_persona_prompt(paths)?;
        if persona.trim().is_empty() {
            Ok(default_system_prompt())
        } else {
            Ok(persona)
        }
    }

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

    pub fn prompts_dir_path(&self, paths: &SaiPaths) -> PathBuf {
        config_relative_path(paths, &self.prompt.prompts_dir)
    }

    pub fn user_identity_path(&self, paths: &SaiPaths) -> PathBuf {
        config_relative_path(paths, &self.prompt.user_identity_file)
    }

    pub fn identities_dir_path(&self, paths: &SaiPaths) -> PathBuf {
        config_relative_path(paths, &self.prompt.identities_dir)
    }

    pub fn persona_path(&self, paths: &SaiPaths, name: &str) -> PathBuf {
        self.prompts_dir_path(paths).join(name)
    }

    pub fn identity_path(&self, paths: &SaiPaths, name: &str) -> PathBuf {
        self.identities_dir_path(paths).join(name)
    }

    pub fn persona_memory_data_dir(&self, paths: &SaiPaths, persona: &str) -> PathBuf {
        paths
            .data_dir
            .join("personas")
            .join(persona_scope_name(persona))
    }

    pub fn persona_memory_state_dir(&self, paths: &SaiPaths, persona: &str) -> PathBuf {
        paths
            .state_dir
            .join("personas")
            .join(persona_scope_name(persona))
    }

    pub fn persona_skills_dir(&self, paths: &SaiPaths, persona: &str) -> PathBuf {
        paths
            .skills_dir
            .join("personas")
            .join(persona_scope_name(persona))
    }

    pub fn active_persona_memory_data_dir(&self, paths: &SaiPaths) -> PathBuf {
        self.persona_memory_data_dir(paths, self.prompt.active_persona.trim())
    }

    pub fn active_persona_memory_state_dir(&self, paths: &SaiPaths) -> PathBuf {
        self.persona_memory_state_dir(paths, self.prompt.active_persona.trim())
    }

    pub fn active_persona_skills_dir(&self, paths: &SaiPaths) -> PathBuf {
        self.persona_skills_dir(paths, self.prompt.active_persona.trim())
    }

    pub fn active_persona_prompt(&self, paths: &SaiPaths) -> Result<String> {
        if !self.prompt.active_persona.trim().is_empty() {
            let path = self.persona_path(paths, self.prompt.active_persona.trim());
            if path.exists() {
                return std::fs::read_to_string(&path)
                    .with_context(|| format!("failed to read {}", path.display()));
            }
        }
        if let Some(prompt) = self
            .system_prompt
            .as_deref()
            .filter(|prompt| !prompt.trim().is_empty())
        {
            return Ok(prompt.to_string());
        }
        let legacy = self.custom_system_prompt(paths)?;
        if legacy.trim().is_empty() {
            Ok(String::new())
        } else {
            Ok(legacy)
        }
    }

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

    pub fn upsert_provider(&mut self, provider: ProviderConfig) {
        self.active_provider = provider.id.clone();
        match self
            .providers
            .iter()
            .position(|item| item.id == provider.id)
        {
            Some(index) => self.providers[index] = provider,
            None => self.providers.push(provider),
        }
    }
}
