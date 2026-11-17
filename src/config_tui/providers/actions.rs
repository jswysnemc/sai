//! 供应商三栏浏览器的增删改与激活状态写操作。

use super::{local_provider_models, ProviderBrowser};
use crate::config::ProviderConfig;
use crate::config_tui::form::add_custom_model_form;
use crate::config_tui::provider_forms::{edit_model_form, edit_provider_form};
use crate::config_tui::ui::{confirm_delete, message};
use crate::i18n::text as t;
use anyhow::Result;
use std::io;

impl ProviderBrowser<'_> {
    /// 【供应商配置】【新增保存】保存供应商草稿；无模型时保留原激活项，返回交互结果。
    ///
    /// 参数:
    /// - `stdout`: 终端输出流
    ///
    /// 返回:
    /// - 执行结果
    pub(super) fn add_provider(&mut self, stdout: &mut io::Stdout) -> Result<()> {
        let mut draft = ProviderConfig::new_openai_compatible();
        draft.id = (1..)
            .map(|index| format!("custom-{index}"))
            .find(|id| self.config.providers.iter().all(|item| &item.id != id))
            .unwrap();
        if let Some(provider) = edit_provider_form(stdout, draft, true)? {
            if self
                .config
                .providers
                .iter()
                .any(|item| item.id == provider.id)
            {
                message(
                    stdout,
                    t("Provider ID already exists", "供应商配置 ID 已存在"),
                )?;
                return Ok(());
            }
            let previous_active = self.config.active_provider.clone();
            let has_model = !local_provider_models(&provider).is_empty();
            self.config.upsert_provider(provider);
            // 1. 【供应商配置】【激活保护】目录失败或为空时允许稍后手动添加模型
            if !has_model {
                self.config.active_provider = previous_active;
            }
            self.provider_idx = self.config.providers.len().saturating_sub(1);
            self.refresh_models();
        }
        Ok(())
    }

    /// 【供应商配置】【删除供应商】二次确认后级联移除供应商及关联模型引用。
    pub(super) fn delete_provider(&mut self, stdout: &mut io::Stdout) -> Result<()> {
        if self.config.providers.is_empty() {
            return Ok(());
        }
        let provider_id = self.config.providers[self.provider_idx].id.clone();
        if !confirm_delete(
            stdout,
            t(" DELETE PROVIDER ", " 删除供应商 "),
            &provider_id,
            t(
                "This also clears models, keys and any model references pointing at it.",
                "同时删除其下的模型与密钥，并清空指向它的模型引用（压缩 / 视觉 / 嵌入 / 子代理）。",
            ),
        )? {
            self.status = t("Delete cancelled", "已取消删除").to_string();
            return Ok(());
        }
        if let Err(error) = self.config.remove_provider(&provider_id) {
            self.status = error.to_string();
            return Ok(());
        }
        self.provider_idx = self
            .provider_idx
            .min(self.config.providers.len().saturating_sub(1));
        self.status = format!("{}: {provider_id}", t("Removed provider", "已删除供应商"));
        self.refresh_models();
        Ok(())
    }

    /// 【供应商配置】【打开编辑】按当前激活栏进入供应商表单或模型详情表单。
    pub(super) fn select_or_edit(&mut self, stdout: &mut io::Stdout) -> Result<()> {
        match self.active_col {
            0 => {
                if let Some(provider) = self.config.providers.get(self.provider_idx).cloned() {
                    if let Some(provider) = edit_provider_form(stdout, provider, false)? {
                        let old_id = self.config.providers[self.provider_idx].id.clone();
                        self.config.providers[self.provider_idx] = provider.clone();
                        if self.config.active_provider == old_id {
                            self.config.active_provider = provider.id.clone();
                        }
                        self.refresh_models();
                    }
                }
            }
            2 => {
                if let (Some(provider), Some(model)) = (
                    self.config.providers.get_mut(self.provider_idx),
                    self.models.get(self.model_idx).cloned(),
                ) {
                    let original = provider.clone();
                    if let Some(metadata) = self.remote_metadata.get(&model.full).cloned() {
                        let current = provider
                            .model_metadata
                            .entry(model.full.clone())
                            .or_default();
                        if current.context_chars.is_none() {
                            current.context_chars = metadata.context_chars;
                        }
                        if current.max_output_tokens.is_none() {
                            current.max_output_tokens = metadata.max_output_tokens;
                        }
                        if current.tags.is_empty() {
                            current.tags = metadata.tags;
                        }
                    }
                    if edit_model_form(stdout, provider, &model.full)? {
                        self.record_model_edit(&model.full);
                    } else {
                        *provider = original;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// 【供应商配置】【模型保存】记录已确认的模型表单修改并同步进可选候选列表。
    pub(super) fn record_model_edit(&mut self, model: &str) {
        let Some(provider) = self.config.providers.get_mut(self.provider_idx) else {
            return;
        };
        if provider.default_model.trim().is_empty() {
            provider.default_model = model.to_string();
        }
        if !provider.models.iter().any(|item| item == model) {
            provider.models.push(model.to_string());
        }
        if provider.enabled && provider.default_model == model {
            self.config.active_provider = provider.id.clone();
        }
        self.status = format!("{}: {model}", t("Updated model settings", "已更新模型设置"));
        self.rebuild_models();
    }

    /// 【供应商配置】【模型激活切换】切换当前聚焦模型是否在 `/model` 候选列表中激活。
    pub(super) fn toggle_model_activation(&mut self) {
        if self.active_col != 2 {
            return;
        }
        let Some(model) = self.models.get(self.model_idx).cloned() else {
            return;
        };
        let Some(provider_id) = self
            .config
            .providers
            .get(self.provider_idx)
            .map(|provider| provider.id.clone())
        else {
            return;
        };
        let is_active = self
            .config
            .providers
            .get(self.provider_idx)
            .map(|provider| provider.models.iter().any(|item| item == &model.full))
            .unwrap_or(false);
        if is_active {
            if self
                .config
                .remove_active_provider_model(&provider_id, &model.full)
                .is_ok()
            {
                self.status = format!(
                    "{}: {}",
                    t("Deactivated model", "已取消激活模型"),
                    model.full
                );
            }
            return;
        }
        if let Some(provider) = self.config.providers.get_mut(self.provider_idx) {
            provider.models.push(model.full.clone());
            if provider.default_model.trim().is_empty() {
                provider.default_model = model.full.clone();
            }
            self.status = format!("{}: {}", t("Activated model", "已激活模型"), model.full);
        }
    }

    /// 【供应商配置】【一键设为当前】在模型栏按 Space 或 u 直接将该模型设为默认并激活供应商。
    pub(super) fn set_current_model(&mut self) {
        if self.active_col != 2 {
            return;
        }
        let Some(model) = self.models.get(self.model_idx).cloned() else {
            return;
        };
        let Some(provider_id) = self
            .config
            .providers
            .get(self.provider_idx)
            .map(|provider| provider.id.clone())
        else {
            return;
        };
        if self
            .config
            .set_active_provider_model(&provider_id, &model.full)
            .is_ok()
        {
            self.status = format!(
                "{}: {provider_id} / {}",
                t("Set active model", "已设为当前模型"),
                model.full
            );
            self.rebuild_models();
        }
    }

    /// 【供应商配置】【新增自定义模型】录入自定义模型标识并定位到新模型行。
    pub(super) fn add_custom_model(&mut self, stdout: &mut io::Stdout) -> Result<()> {
        let Some(model) = add_custom_model_form(stdout)? else {
            return Ok(());
        };
        let Some(provider) = self.config.providers.get_mut(self.provider_idx) else {
            return Ok(());
        };
        if provider.models.iter().any(|item| item == &model) {
            self.status = format!("{}: {model}", t("Model already exists", "模型已存在"));
            return Ok(());
        }
        provider.models.push(model.clone());
        if provider.default_model.trim().is_empty() {
            provider.default_model = model.clone();
        }
        if !self.raw_models.iter().any(|item| item == &model) {
            self.raw_models.push(model.clone());
        }
        self.rebuild_models();
        self.model_idx = self
            .models
            .iter()
            .position(|entry| entry.full == model)
            .unwrap_or(self.model_idx);
        self.status = format!("{}: {model}", t("Added custom model", "已添加自定义模型"));
        Ok(())
    }

    /// 【供应商配置】【移除模型】二次确认后移除当前高亮模型及关联元数据。
    pub(super) fn delete_model(&mut self, stdout: &mut io::Stdout) -> Result<()> {
        let Some(model) = self
            .models
            .get(self.model_idx)
            .map(|entry| entry.full.clone())
        else {
            return Ok(());
        };
        let Some(provider_id) = self
            .config
            .providers
            .get(self.provider_idx)
            .map(|provider| provider.id.clone())
        else {
            return Ok(());
        };
        if !confirm_delete(
            stdout,
            t(" REMOVE MODEL ", " 移除模型 "),
            &model,
            t(
                "Removes it from this provider's model list.",
                "从该供应商的模型列表中移除。",
            ),
        )? {
            self.status = t("Delete cancelled", "已取消删除").to_string();
            return Ok(());
        }
        if self
            .config
            .remove_active_provider_model(&provider_id, &model)
            .is_ok()
        {
            self.rebuild_models();
            self.status = format!("{}: {model}", t("Removed model", "已移除模型"));
        }
        Ok(())
    }
}
