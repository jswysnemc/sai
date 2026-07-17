use crate::config::{AppConfig, ModelMetadata, ProviderConfig};
use crate::i18n::text as t;
use anyhow::Result;
use crossterm::cursor::MoveTo;
use crossterm::event::KeyCode;
use crossterm::queue;
use crossterm::style::Print;
use crossterm::terminal::{self, Clear, ClearType};
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use super::input::{read_key, read_key_with_timeout};
use super::provider_fetch::{fetch_models, FetchModelsResult};
use super::provider_forms::{edit_model_form, edit_provider_form};
use super::ui::{draw_column, draw_menu, message, truncate};

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
        }
    }

    pub(crate) fn run(mut self, stdout: &mut io::Stdout) -> Result<()> {
        self.refresh_models();
        loop {
            self.poll_fetch_result();
            self.draw(stdout)?;
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
                        self.filter.clear();
                        self.rebuild_models();
                    }
                    KeyCode::Char('r') => self.refresh_models(),
                    KeyCode::Char('a') => self.add_provider(stdout)?,
                    KeyCode::Char('d') => self.delete_provider(),
                    KeyCode::Tab if self.active_col == 2 => self.toggle_model_activation(),
                    KeyCode::Enter | KeyCode::Char('i') => self.select_or_edit(stdout)?,
                    _ => {}
                },
            }
        }
    }

    fn handle_filter_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Esc => {
                self.filter_mode = false;
                self.filter.clear();
            }
            KeyCode::Enter => self.filter_mode = false,
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

    fn refresh_models(&mut self) {
        self.provider_idx = self
            .provider_idx
            .min(self.config.providers.len().saturating_sub(1));
        self.raw_models.clear();
        self.orgs = vec!["All".to_string()];
        self.models.clear();
        self.fetch_seq += 1;
        if let Some(provider) = self.config.providers.get(self.provider_idx).cloned() {
            let seq = self.fetch_seq;
            let (tx, rx) = mpsc::channel();
            self.fetch_rx = Some(rx);
            self.loading = true;
            self.status = t("Fetching model list...", "正在获取模型列表...").to_string();
            std::thread::spawn(move || {
                let result = fetch_models(&provider).map_err(|err| err.to_string());
                let _ = tx.send((seq, result));
            });
        } else {
            self.fetch_rx = None;
            self.loading = false;
            self.status.clear();
        }
        self.org_idx = 0;
        self.model_idx = 0;
    }

    fn poll_fetch_result(&mut self) {
        let Some(rx) = &self.fetch_rx else {
            return;
        };
        let Ok((seq, result)) = rx.try_recv() else {
            return;
        };
        if seq != self.fetch_seq {
            return;
        }
        self.loading = false;
        self.fetch_rx = None;
        match result {
            Ok(result) => {
                self.status = format!(
                    "{} {} {}",
                    t("Fetched", "已获取"),
                    result.models.len(),
                    t("models", "个模型")
                );
                self.raw_models = result.models;
                self.remote_metadata = result.metadata;
            }
            Err(err) => {
                self.status = format_status_line(&format!(
                    "{}: {err}",
                    t("Failed to fetch models", "获取模型失败")
                ));
                self.raw_models.clear();
            }
        }
        self.rebuild_models();
    }

    fn rebuild_models(&mut self) {
        let filter = self.filter.to_ascii_lowercase();
        let mut grouped: BTreeMap<String, Vec<ModelEntry>> = BTreeMap::new();
        for model in &self.raw_models {
            if !filter.is_empty() && !model.to_ascii_lowercase().contains(&filter) {
                continue;
            }
            let org = model
                .split_once('/')
                .map(|(org, _)| org)
                .unwrap_or("All")
                .to_string();
            let name = model
                .split_once('/')
                .map(|(_, name)| name)
                .unwrap_or(model)
                .to_string();
            grouped
                .entry("All".to_string())
                .or_default()
                .push(ModelEntry::new(model, model));
            if org != "All" {
                grouped
                    .entry(org)
                    .or_default()
                    .push(ModelEntry::new(&name, model));
            }
        }
        self.orgs = grouped.keys().cloned().collect();
        if self.orgs.is_empty() {
            self.orgs.push("All".to_string());
        }
        self.org_idx = self.org_idx.min(self.orgs.len().saturating_sub(1));
        self.models = grouped.remove(&self.orgs[self.org_idx]).unwrap_or_default();
        self.model_idx = self.model_idx.min(self.models.len().saturating_sub(1));
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

    fn delete_provider(&mut self) {
        if self.config.providers.is_empty() {
            return;
        }
        let provider_id = self.config.providers[self.provider_idx].id.clone();
        if let Err(error) = self.config.remove_provider(&provider_id) {
            self.status = error.to_string();
            return;
        }
        self.provider_idx = self
            .provider_idx
            .min(self.config.providers.len().saturating_sub(1));
        self.refresh_models();
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
                    if let Some(metadata) = self.remote_metadata.get(&model.full).cloned() {
                        let current = provider
                            .model_metadata
                            .entry(model.full.clone())
                            .or_default();
                        if current.context_chars.is_none() {
                            current.context_chars = metadata.context_chars;
                        }
                        if current.tags.is_empty() {
                            current.tags = metadata.tags;
                        }
                    }
                    if edit_model_form(stdout, provider, &model.full)? {
                        self.config.active_provider = provider.id.clone();
                        self.status = format!(
                            "{}: {}",
                            t("Updated model settings", "已更新模型设置"),
                            model.full
                        );
                    }
                }
            }
            _ => {}
        }
        Ok(())
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

    fn draw(&self, stdout: &mut io::Stdout) -> Result<()> {
        let (cols, rows) = terminal::size()?;
        let inner_x = 0;
        let inner_y = 0;
        let inner_w = cols;
        let inner_h = rows.saturating_sub(2);
        let left_w = inner_w.saturating_mul(28).saturating_div(100).max(20);
        let mid_w = inner_w.saturating_mul(22).saturating_div(100).max(16);
        let right_w = inner_w
            .saturating_sub(left_w)
            .saturating_sub(mid_w)
            .saturating_sub(2)
            .max(18);
        let providers = self
            .config
            .providers
            .iter()
            .map(|provider| {
                let active = if provider.id == self.config.active_provider {
                    "* "
                } else {
                    "  "
                };
                format!("{active}{}", provider.display_name)
            })
            .collect::<Vec<_>>();
        let models = self
            .models
            .iter()
            .map(|model| {
                let current = self
                    .config
                    .providers
                    .get(self.provider_idx)
                    .map(|provider| provider.default_model == model.full)
                    .unwrap_or(false);
                let active = self
                    .config
                    .providers
                    .get(self.provider_idx)
                    .map(|provider| provider.models.iter().any(|item| item == &model.full))
                    .unwrap_or(false);
                if current && active {
                    format!(
                        "{} [{} {}]",
                        model.name,
                        t("current", "当前"),
                        t("active", "激活")
                    )
                } else if current {
                    format!("{} [{}]", model.name, t("current", "当前"))
                } else if active {
                    format!("{} [{}]", model.name, t("active", "激活"))
                } else {
                    model.name.clone()
                }
            })
            .collect::<Vec<_>>();

        queue!(stdout, Clear(ClearType::All))?;
        draw_column(
            stdout,
            inner_x,
            inner_y,
            left_w,
            inner_h,
            t(" PROVIDERS ", " 供应商 "),
            &providers,
            self.provider_idx,
            self.active_col == 0,
        )?;
        draw_column(
            stdout,
            inner_x + left_w + 1,
            inner_y,
            mid_w,
            inner_h,
            t(" ORG ", " 组织 "),
            &self.orgs,
            self.org_idx,
            self.active_col == 1,
        )?;
        let title = if self.filter.is_empty() {
            t(" MODELS ", " 模型 ").to_string()
        } else {
            format!("{} /{} ", t(" MODELS", " 模型"), self.filter)
        };
        draw_column(
            stdout,
            inner_x + left_w + mid_w + 2,
            inner_y,
            right_w,
            inner_h,
            &title,
            &models,
            self.model_idx,
            self.active_col == 2,
        )?;
        let help = if self.filter_mode {
            format!(
                "{}: {}_  {}",
                t("Search", "搜索"),
                self.filter,
                t("[Enter] confirm [Esc] cancel", "[Enter]确认 [Esc]取消")
            )
        } else {
            t(
                "[h/l] columns [j/k] move [Tab] activate model [Enter] model settings [/] search [r] refresh [a] add [d] delete [q] back",
                "[h/l]切栏 [j/k]移动 [Tab]激活模型 [Enter]模型设置 [/]搜索 [r]刷新 [a]添加 [d]删除 [q]返回",
            )
            .to_string()
        };
        let status = if self.loading {
            format!("{}", self.status)
        } else {
            self.status.clone()
        };
        queue!(
            stdout,
            MoveTo(0, rows.saturating_sub(2)),
            Clear(ClearType::CurrentLine),
            Print(truncate(&status, cols as usize))
        )?;
        queue!(
            stdout,
            MoveTo(0, rows.saturating_sub(1)),
            Clear(ClearType::CurrentLine),
            Print(truncate(&help, cols as usize))
        )?;
        stdout.flush()?;
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
        draw_menu(
            stdout,
            t(" SELECT PROVIDER/MODEL ", " 选择供应商/模型 "),
            &options,
            selected,
            t(
                "[Enter] select [d] remove [q] back",
                "[Enter]选择 [d]移除 [q]返回",
            ),
        )?;
        match read_key()? {
            KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
            KeyCode::Up | KeyCode::Char('k') => selected = selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => selected = (selected + 1).min(options.len() - 1),
            KeyCode::Char('d') => {
                // 移除当前高亮模型（含元数据），并刷新列表
                let choice = &choices[selected];
                let provider_id = choice.provider_id.clone();
                let model = choice.model.clone();
                config.remove_active_provider_model(&provider_id, &model)?;
                choices = config.provider_model_choices();
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
