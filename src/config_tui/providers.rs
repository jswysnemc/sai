mod catalog;
mod render;

use crate::config::{AppConfig, ModelMetadata, ProviderConfig};
use crate::i18n::text as t;
use anyhow::Result;
use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::KeyCode;
use crossterm::queue;
use crossterm::style::Print;
use crossterm::terminal::{self, Clear, ClearType};
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use super::form::add_custom_model_form;
use super::input::{read_key, read_key_with_timeout};
use super::layout::three_column_widths;
use super::provider_fetch::{fetch_models, FetchModelsResult};
use super::provider_forms::{edit_model_form, edit_provider_form};
use super::ui::{confirm_delete, display_width, draw_column, draw_menu, message, pad, truncate};

pub(crate) struct ProviderBrowser<'a> {
    config: &'a mut AppConfig,
    active_col: usize,
    provider_idx: usize,
    org_idx: usize,
    model_idx: usize,
    filter: String,
    filter_mode: bool,
    raw_models: Vec<String>,
    remote_metadata: BTreeMap<String, ModelMetadata>,
    orgs: Vec<String>,
    models: Vec<ModelEntry>,
    status: String,
    loading: bool,
    fetch_seq: u64,
    fetch_rx: Option<Receiver<FetchResult>>,
    /// 光标是否已处于显示态；避免逐帧重发 Show/Hide 放大闪烁
    cursor_visible: bool,
}

impl<'a> ProviderBrowser<'a> {
    pub(crate) fn new(config: &'a mut AppConfig) -> Self {
        Self {
            config,
            active_col: 0,
            provider_idx: 0,
            org_idx: 0,
            model_idx: 0,
            filter: String::new(),
            filter_mode: false,
            raw_models: Vec::new(),
            remote_metadata: BTreeMap::new(),
            orgs: Vec::new(),
            models: Vec::new(),
            status: String::new(),
            loading: false,
            fetch_seq: 0,
            fetch_rx: None,
            cursor_visible: false,
        }
    }

    pub(crate) fn run(mut self, stdout: &mut io::Stdout) -> Result<()> {
        self.refresh_models();
        // 上一帧签名：轮询 tick 里内容没变就不重绘，避免加载期间
        // 10Hz 清屏重画造成的整屏闪烁
        let mut last_frame: Option<String> = None;
        loop {
            let before = self.frame_signature();
            self.poll_fetch_result();
            let changed = before != self.frame_signature();
            if changed || last_frame.is_none() {
                self.draw(stdout)?;
                last_frame = Some(self.frame_signature());
            }
            match read_key_with_timeout(if self.loading {
                Some(Duration::from_millis(100))
            } else {
                None
            })? {
                None => continue,
                Some(key) => match key {
                    key if self.filter_mode => self.handle_filter_key(key),
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Left | KeyCode::Char('h') => self.move_left(),
                    KeyCode::Right | KeyCode::Char('l') => self.move_right(),
                    KeyCode::Up | KeyCode::Char('k') => self.move_up(),
                    KeyCode::Down | KeyCode::Char('j') => self.move_down(),
                    KeyCode::Char('/') => {
                        self.filter_mode = true;
                        self.active_col = 2;
                    }
                    KeyCode::Char('r') => self.refresh_models(),
                    KeyCode::Char('a') if self.active_col == 2 => self.add_custom_model(stdout)?,
                    KeyCode::Char('a') => self.add_provider(stdout)?,
                    KeyCode::Char('d') if self.active_col == 2 => self.delete_model(stdout)?,
                    KeyCode::Char('d') => self.delete_provider(stdout)?,
                    KeyCode::Tab if self.active_col == 2 => self.toggle_model_activation(),
                    KeyCode::Enter | KeyCode::Char('i') => self.select_or_edit(stdout)?,
                    _ => {}
                },
            }
        }
    }

    /// 计算当前界面的内容签名。
    ///
    /// 渲染涉及的全部状态拼接为字符串，用于轮询 tick 的脏检查：
    /// loading 等待期 100ms 一拍，若签名未变则跳过整屏重绘。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 内容签名
    fn frame_signature(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            self.active_col,
            self.provider_idx,
            self.org_idx,
            self.model_idx,
            self.filter,
            self.filter_mode,
            self.loading,
            self.status,
            self.orgs.len(),
            self.models.len(),
            self.config.providers.len(),
        )
    }

    fn handle_filter_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Esc => {
                self.filter_mode = false;
                self.filter.clear();
            }
            KeyCode::Enter | KeyCode::Tab => self.filter_mode = false,
            KeyCode::Up => {
                self.move_up();
                return;
            }
            KeyCode::Down => {
                self.move_down();
                return;
            }
            KeyCode::Backspace => {
                self.filter.pop();
            }
            KeyCode::Char(ch) => self.filter.push(ch),
            _ => {}
        }
        self.rebuild_models();
    }

    fn move_left(&mut self) {
        self.active_col = self.active_col.saturating_sub(1);
    }

    fn move_right(&mut self) {
        self.active_col = (self.active_col + 1).min(2);
    }

    fn move_up(&mut self) {
        match self.active_col {
            0 => {
                self.provider_idx = self.provider_idx.saturating_sub(1);
                self.refresh_models();
            }
            1 => {
                self.org_idx = self.org_idx.saturating_sub(1);
                self.rebuild_models();
            }
            2 => self.model_idx = self.model_idx.saturating_sub(1),
            _ => {}
        }
    }

    fn move_down(&mut self) {
        match self.active_col {
            0 => {
                self.provider_idx =
                    (self.provider_idx + 1).min(self.config.providers.len().saturating_sub(1));
                self.refresh_models();
            }
            1 => {
                self.org_idx = (self.org_idx + 1).min(self.orgs.len().saturating_sub(1));
                self.rebuild_models();
            }
            2 => self.model_idx = (self.model_idx + 1).min(self.models.len().saturating_sub(1)),
            _ => {}
        }
    }

    fn add_provider(&mut self, stdout: &mut io::Stdout) -> Result<()> {
        if let Some(provider) = edit_provider_form(stdout, ProviderConfig::new_openai_compatible())?
        {
            self.config.upsert_provider(provider);
            self.provider_idx = self.config.providers.len().saturating_sub(1);
            self.refresh_models();
        }
        Ok(())
    }

    fn delete_provider(&mut self, stdout: &mut io::Stdout) -> Result<()> {
        if self.config.providers.is_empty() {
            return Ok(());
        }
        let provider_id = self.config.providers[self.provider_idx].id.clone();
        // 删除会级联清理压缩/视觉/嵌入/subagent 的模型引用，且无法撤销，
        // 必须先确认，并在副标题里说明后果
        if !confirm_delete(
            stdout,
            &t(" DELETE PROVIDER ", " 删除供应商 "),
            &provider_id,
            &t(
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

    fn select_or_edit(&mut self, stdout: &mut io::Stdout) -> Result<()> {
        match self.active_col {
            0 => {
                if let Some(provider) = self.config.providers.get(self.provider_idx).cloned() {
                    if let Some(provider) = edit_provider_form(stdout, provider)? {
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

    /// 【配置】【模型保存】记录已确认的模型表单修改。
    ///
    /// 参数: `model` 为当前供应商下保存的模型标识
    /// 返回: 无；登记可选模型并更新界面状态
    fn record_model_edit(&mut self, model: &str) {
        let Some(provider) = self.config.providers.get_mut(self.provider_idx) else {
            return;
        };
        if provider.default_model.trim().is_empty() {
            provider.default_model = model.to_string();
        }
        // 【配置】【模型刷新】显式保存的模型进入候选列表，不依赖是否设为默认模型
        if !provider.models.iter().any(|item| item == model) {
            provider.models.push(model.to_string());
        }
        if provider.enabled && provider.default_model == model {
            self.config.active_provider = provider.id.clone();
        }
        self.status = format!("{}: {model}", t("Updated model settings", "已更新模型设置"));
        self.rebuild_models();
    }

    fn toggle_model_activation(&mut self) {
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
            // 通过统一移除接口清理列表与元数据
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

    /// 在当前供应商下新增自定义模型。
    fn add_custom_model(&mut self, stdout: &mut io::Stdout) -> Result<()> {
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

    /// 删除当前选中的本地模型和关联元数据。
    fn delete_model(&mut self, stdout: &mut io::Stdout) -> Result<()> {
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
            &t(" REMOVE MODEL ", " 移除模型 "),
            &model,
            &t(
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

type FetchResult = (u64, Result<FetchModelsResult, String>);

fn format_status_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[derive(Clone)]
struct ModelEntry {
    name: String,
    full: String,
}

/// 判断模型标识是否命中过滤词（大小写不敏感子串）。
///
/// 参数:
/// - `model`: 完整模型标识
/// - `filter`: 已规范化或原始的过滤词
///
/// 返回:
/// - 空过滤词视为全部命中
fn model_matches_filter(model: &str, filter: &str) -> bool {
    let filter = filter.trim();
    filter.is_empty()
        || model
            .to_ascii_lowercase()
            .contains(&filter.to_ascii_lowercase())
}

/// 返回当前供应商的本地模型，默认模型和已激活模型优先。
fn local_provider_models(provider: &ProviderConfig) -> Vec<String> {
    let mut models = Vec::new();
    if !provider.default_model.trim().is_empty() {
        models.push(provider.default_model.clone());
    }
    for model in &provider.models {
        if !models.iter().any(|item| item == model) {
            models.push(model.clone());
        }
    }
    models
}

impl ModelEntry {
    fn new(name: &str, full: &str) -> Self {
        Self {
            name: name.to_string(),
            full: full.to_string(),
        }
    }
}

pub(crate) fn select_active_provider(
    stdout: &mut io::Stdout,
    config: &mut AppConfig,
) -> Result<()> {
    let mut choices = config.provider_model_choices();
    if choices.is_empty() {
        message(
            stdout,
            t(
                "No available Provider, add one first.",
                "没有可用 Provider，请先添加。",
            ),
        )?;
        return Ok(());
    }
    let mut selected = choices
        .iter()
        .position(|choice| {
            config
                .provider(None)
                .map(|provider| {
                    provider.id == choice.provider_id && provider.default_model == choice.model
                })
                .unwrap_or(false)
        })
        .unwrap_or(0);
    let mut status = String::new();
    loop {
        if choices.is_empty() {
            message(
                stdout,
                t(
                    "No available Provider models left.",
                    "已无可用 Provider 模型。",
                ),
            )?;
            return Ok(());
        }
        selected = selected.min(choices.len().saturating_sub(1));
        let options = choices
            .iter()
            .map(|choice| choice.label())
            .collect::<Vec<_>>();
        let help = if status.is_empty() {
            t(
                "[Enter] select [d] remove [q] back",
                "[Enter]选择 [d]移除 [q]返回",
            )
            .to_string()
        } else {
            status.clone()
        };
        draw_menu(
            stdout,
            t(" SELECT PROVIDER/MODEL ", " 选择供应商/模型 "),
            &options,
            selected,
            &help,
        )?;
        match read_key()? {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
            KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => selected = (selected + 1).min(options.len() - 1),
            KeyCode::Char('d') => {
                // 移除当前高亮模型（含元数据），失败时在菜单内提示而不终止 TUI
                let choice = &choices[selected];
                let provider_id = choice.provider_id.clone();
                let model = choice.model.clone();
                match config.remove_active_provider_model(&provider_id, &model) {
                    Ok(()) => {
                        status.clear();
                        choices = config.provider_model_choices();
                    }
                    Err(err) => status = err.to_string(),
                }
            }
            KeyCode::Enter => {
                config.set_active_provider_model(
                    &choices[selected].provider_id,
                    &choices[selected].model,
                )?;
                return Ok(());
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 【配置】【模型刷新测试】保存远程模型设置并重新加载后，模型选择器应能找到该模型。
    #[test]
    fn regression_saved_model_is_available_after_returning_to_repl() {
        let temp = tempfile::tempdir().unwrap();
        let paths = crate::paths::SaiPaths::for_tests(temp.path());
        let mut config = AppConfig::default();
        let provider_id = config.providers[0].id.clone();
        config.providers[0].models = vec!["existing-model".into()];
        config.providers[0].default_model = "existing-model".into();
        let metadata = config.providers[0]
            .model_metadata
            .entry("new-model".into())
            .or_default();
        metadata.max_output_tokens = Some(4096);
        ProviderBrowser::new(&mut config).record_model_edit("new-model");
        config.save(&paths).unwrap();
        AppConfig::init_files(&paths).unwrap();
        let reloaded = AppConfig::load(&paths).unwrap();
        assert!(
            reloaded
                .provider_model_choices()
                .iter()
                .any(|choice| { choice.provider_id == provider_id && choice.model == "new-model" }),
            "保存的模型未出现在 /model 候选中"
        );
    }

    /// 空过滤词不过滤；大小写与子串都能命中。
    #[test]
    fn model_filter_matches_substring_case_insensitively() {
        assert!(model_matches_filter("minimax-m3", ""));
        assert!(model_matches_filter("minimax-m3", "Mini"));
        assert!(model_matches_filter("openai/gpt-4o", "gpt-4"));
        assert!(!model_matches_filter("minimax-m3", "claude"));
    }
}
