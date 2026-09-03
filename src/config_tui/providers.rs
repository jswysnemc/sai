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

    fn refresh_models(&mut self) {
        self.provider_idx = self
            .provider_idx
            .min(self.config.providers.len().saturating_sub(1));
        self.raw_models = self
            .config
            .providers
            .get(self.provider_idx)
            .map(local_provider_models)
            .unwrap_or_default();
        self.orgs = vec!["All".to_string()];
        self.models.clear();
        self.rebuild_models();
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
                for model in result.models {
                    if !self.raw_models.iter().any(|item| item == &model) {
                        self.raw_models.push(model);
                    }
                }
                self.remote_metadata = result.metadata;
            }
            Err(err) => {
                self.status = format_status_line(&format!(
                    "{}: {err}",
                    t("Failed to fetch models", "获取模型失败")
                ));
            }
        }
        self.rebuild_models();
    }

    fn rebuild_models(&mut self) {
        let mut grouped: BTreeMap<String, Vec<ModelEntry>> = BTreeMap::new();
        let ordered_models = self.prioritized_models();
        for model in &ordered_models {
            if !model_matches_filter(model, &self.filter) {
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
                        // 只改标签/上下文时旧逻辑也会把该供应商设为当前项，
                        // 若此时还没有 default_model，加载会回退到已停用的 opencode。
                        if provider.default_model.trim().is_empty() {
                            provider.default_model = model.full.clone();
                            if !provider.models.iter().any(|item| item == &model.full) {
                                provider.models.push(model.full.clone());
                            }
                        }
                        if provider.enabled && provider.default_model == model.full {
                            self.config.active_provider = provider.id.clone();
                        }
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

    /// 返回本地已激活模型优先的合并列表。
    fn prioritized_models(&self) -> Vec<String> {
        let mut models = self
            .config
            .providers
            .get(self.provider_idx)
            .map(local_provider_models)
            .unwrap_or_default();
        for model in &self.raw_models {
            if !models.iter().any(|item| item == model) {
                models.push(model.clone());
            }
        }
        models
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

    fn draw(&mut self, stdout: &mut io::Stdout) -> Result<()> {
        let (cols, rows) = terminal::size()?;
        let frame = super::layout::full_frame(cols, rows);
        // 外框内缩：左右各留边框 + 一格边距，底部留状态行与嵌入式帮助条
        let inner_x = frame.x.saturating_add(2);
        let inner_y = frame.y.saturating_add(1);
        let inner_w = frame.width.saturating_sub(4);
        let inner_h = frame.height.saturating_sub(4);
        let providers = self
            .config
            .providers
            .iter()
            .map(|provider| {
                let active = if provider.id == self.config.active_provider {
                    "● "
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

        let models_title = t(" MODELS ", " 模型 ").to_string();
        queue!(stdout, Clear(ClearType::All))?;
        super::ui::draw_box(
            stdout,
            frame.x,
            frame.y,
            frame.width,
            frame.height,
            t("PROVIDERS & MODELS", "供应商与模型"),
        )?;
        let filter_cursor = self.draw_filter_bar(stdout, inner_x, inner_y, inner_w)?;
        let list_y = inner_y.saturating_add(2);
        let list_h = inner_h.saturating_sub(2);
        // 1. 终端够宽时绘制三栏；宽度之和不超过内容区，保证不重叠
        if let Some((left_w, mid_w, right_w)) = three_column_widths(inner_w) {
            draw_column(
                stdout,
                inner_x,
                list_y,
                left_w,
                list_h,
                t(" PROVIDERS ", " 供应商 "),
                &providers,
                self.provider_idx,
                self.active_col == 0,
            )?;
            draw_column(
                stdout,
                inner_x + left_w + 1,
                list_y,
                mid_w,
                list_h,
                t(" ORG ", " 组织 "),
                &self.orgs,
                self.org_idx,
                self.active_col == 1,
            )?;
            draw_column(
                stdout,
                inner_x + left_w + mid_w + 2,
                list_y,
                right_w,
                list_h,
                &models_title,
                &models,
                self.model_idx,
                self.active_col == 2,
            )?;
        } else {
            // 2. 窄终端降级为单栏，仅绘制当前激活栏
            let (title, items, selected) = match self.active_col {
                0 => (
                    t(" PROVIDERS ", " 供应商 ").to_string(),
                    &providers,
                    self.provider_idx,
                ),
                1 => (t(" ORG ", " 组织 ").to_string(), &self.orgs, self.org_idx),
                _ => (models_title, &models, self.model_idx),
            };
            draw_column(
                stdout, inner_x, list_y, inner_w, list_h, &title, items, selected, true,
            )?;
        }
        use super::theme::{help_line, ACCENT, DANGER, MUTED, RESET};
        let help = if self.filter_mode {
            help_line(&[
                ("type", t("filter models", "过滤模型")),
                ("↑↓", t("move", "移动")),
                ("Enter", t("keep filter", "保留过滤")),
                ("Esc", t("clear", "清除")),
            ])
        } else {
            // 删除是破坏性操作，键名用警示色与其它键区分
            let base = help_line(&[
                ("h/l", t("columns", "切栏")),
                ("j/k", t("move", "移动")),
                ("Tab", t("activate", "激活模型")),
                ("Enter", t("settings", "模型设置")),
                ("/", t("search", "搜索")),
                ("r", t("refresh", "刷新")),
                ("a", t("add", "添加")),
            ]);
            format!(
                "{base}{MUTED} · {RESET}{DANGER}d{RESET} {MUTED}{}{RESET}{MUTED} · {RESET}{ACCENT}q{RESET} {MUTED}{}{RESET}",
                t("delete", "删除"),
                t("back", "返回")
            )
        };
        // 状态行放在框内底部，弱化显示，不与内容抢视线
        queue!(
            stdout,
            MoveTo(inner_x, rows.saturating_sub(2)),
            Print(format!(
                "{MUTED}{}{RESET}",
                truncate(&self.status, inner_w as usize)
            ))
        )?;
        super::ui::draw_status_bar(stdout, &frame, &help)?;
        // 光标显隐只在过滤态切换后的首帧发送，逐帧重发会放大终端闪烁
        if let Some((cx, cy)) = filter_cursor {
            if !self.cursor_visible {
                queue!(stdout, Show)?;
                self.cursor_visible = true;
            }
            queue!(stdout, MoveTo(cx, cy))?;
        } else if self.cursor_visible {
            queue!(stdout, Hide)?;
            self.cursor_visible = false;
        }
        stdout.flush()?;
        Ok(())
    }

    /// 在三栏上方画一条全宽过滤框，避免查询被挤进窄列标题。
    ///
    /// 参数:
    /// - `stdout`: 终端输出
    /// - `x`: 内容区左列
    /// - `y`: 过滤框行
    /// - `width`: 内容区宽度
    ///
    /// 返回:
    /// - 过滤态下的光标坐标
    fn draw_filter_bar(
        &self,
        stdout: &mut io::Stdout,
        x: u16,
        y: u16,
        width: u16,
    ) -> Result<Option<(u16, u16)>> {
        use super::theme::{ACCENT, BOLD, MUTED, RESET, SELECT_BG};
        let prefix = format!("/ {}", t("Search models", "搜索模型"));
        let query = if self.filter.is_empty() && !self.filter_mode {
            t("type to filter", "输入以过滤")
        } else {
            self.filter.as_str()
        };
        let count = if self.filter.is_empty() {
            String::new()
        } else {
            format!("  {} {}", self.models.len(), t("matches", "项"))
        };
        let field_w = (width as usize).saturating_sub(display_width(&count));
        let body = truncate(&format!("{prefix}  {query}"), field_w.saturating_sub(1));
        let line = format!("{}{count}", pad(&body, field_w.saturating_sub(1)));
        if self.filter_mode {
            queue!(
                stdout,
                MoveTo(x, y),
                Print(format!("{SELECT_BG}{ACCENT}{BOLD} {line}{RESET}"))
            )?;
            let cursor_x = x
                + 1
                + display_width(&truncate(
                    &format!("{prefix}  {}", self.filter),
                    field_w.saturating_sub(1),
                )) as u16;
            Ok(Some((
                cursor_x.min(x.saturating_add(width.saturating_sub(1))),
                y,
            )))
        } else {
            queue!(
                stdout,
                MoveTo(x, y),
                Print(format!("{MUTED} {line}{RESET}"))
            )?;
            Ok(None)
        }
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
    use super::model_matches_filter;

    /// 空过滤词不过滤；大小写与子串都能命中。
    #[test]
    fn model_filter_matches_substring_case_insensitively() {
        assert!(model_matches_filter("minimax-m3", ""));
        assert!(model_matches_filter("minimax-m3", "Mini"));
        assert!(model_matches_filter("openai/gpt-4o", "gpt-4"));
        assert!(!model_matches_filter("minimax-m3", "claude"));
    }
}
