//! 供应商与模型三栏浏览器主状态机：驱动三栏导航、过滤游标、粘贴检索与两段式 Esc 清空。

mod actions;
mod active_selector;
mod catalog;
#[cfg(test)]
mod catalog_tests;
mod mouse;
mod render;

pub(crate) use active_selector::select_active_provider;

use crate::config::{AppConfig, ModelMetadata, ProviderConfig};
use anyhow::Result;
use crossterm::event::KeyCode;
use crossterm::terminal;
use std::collections::BTreeMap;
use std::io;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use super::input::{read_event_with_timeout, sanitize_paste_single_line, ConfigEvent};
use super::layout::three_column_widths;
use super::mouse::ClickTracker;
use super::provider_fetch::{fetch_models, FetchModelsResult};
use super::search::{
    clamp_index, insert_char_at, insert_str_at_char, remove_char_at, remove_char_before,
};
use super::ui::{display_width, pad, truncate};

pub(crate) struct ProviderBrowser<'a> {
    config: &'a mut AppConfig,
    active_col: usize,
    provider_idx: usize,
    org_idx: usize,
    model_idx: usize,
    hovered: Option<mouse::ProviderClick>,
    hovered_footer: Option<super::mouse::HitRect>,
    filter: String,
    filter_cursor: usize,
    filter_mode: bool,
    raw_models: Vec<String>,
    remote_metadata: BTreeMap<String, ModelMetadata>,
    orgs: Vec<String>,
    models: Vec<ModelEntry>,
    status: String,
    loading: bool,
    pending_fetches: BTreeMap<String, Receiver<Result<FetchModelsResult, String>>>,
    /// 按供应商配置指纹缓存结果，切回相同配置时复用目录
    model_cache: BTreeMap<String, Result<FetchModelsResult, String>>,
    /// 光标是否已处于显示态；避免逐帧重发 Show/Hide 放大闪烁
    cursor_visible: bool,
}

impl Drop for ProviderBrowser<'_> {
    /// 【供应商配置】【输入法恢复】离开供应商浏览器时若仍处搜索编辑态则关闭输入法。
    fn drop(&mut self) {
        if self.filter_mode {
            super::ime::disable_for_nav();
        }
    }
}

impl<'a> ProviderBrowser<'a> {
    /// 【供应商配置】【初始化】构造供应商三栏浏览器实例。
    ///
    /// 参数:
    /// - `config`: 当前会话配置可变引用
    ///
    /// 返回:
    /// - 供应商浏览器状态机
    pub(crate) fn new(config: &'a mut AppConfig) -> Self {
        Self {
            config,
            active_col: 0,
            provider_idx: 0,
            org_idx: 0,
            model_idx: 0,
            hovered: None,
            hovered_footer: None,
            filter: String::new(),
            filter_cursor: 0,
            filter_mode: false,
            raw_models: Vec::new(),
            remote_metadata: BTreeMap::new(),
            orgs: Vec::new(),
            models: Vec::new(),
            status: String::new(),
            loading: false,
            pending_fetches: BTreeMap::new(),
            model_cache: BTreeMap::new(),
            cursor_visible: false,
        }
    }

    /// 【供应商配置】【进入过滤】打开模型过滤栏、定位光标并激活系统输入法。
    pub(super) fn enter_filter_mode(&mut self) {
        self.filter_mode = true;
        self.active_col = 2;
        self.filter_cursor = self.filter.chars().count();
        super::ime::enable_for_edit();
    }

    /// 【供应商配置】【退出过滤编辑】退出过滤输入态并恢复导航态输入法关闭策略。
    pub(super) fn leave_filter_mode(&mut self) {
        if self.filter_mode {
            self.filter_mode = false;
            super::ime::disable_for_nav();
        }
    }

    /// 【供应商配置】【主循环】驱动三栏绘制、异步目录轮询、过滤粘贴与键盘鼠标事件。
    ///
    /// 参数:
    /// - `stdout`: 终端输出流
    ///
    /// 返回:
    /// - 浏览器退出结果
    pub(crate) fn run(mut self, stdout: &mut io::Stdout) -> Result<()> {
        self.refresh_models();
        let mut last_frame: Option<String> = None;
        let mut last_hitbox = mouse::ProviderHitbox::default();
        let mut clicks = ClickTracker::default();
        loop {
            self.poll_fetch_result();
            let signature = self.frame_signature();
            if last_frame.as_ref() != Some(&signature) {
                last_hitbox = self.draw(stdout)?;
                last_frame = Some(signature);
            }
            let timeout = if self.loading {
                Some(Duration::from_millis(100))
            } else {
                Some(Duration::from_millis(150))
            };
            let key = match read_event_with_timeout(timeout)? {
                None => continue,
                Some(ConfigEvent::Resize) => {
                    last_frame = None;
                    continue;
                }
                Some(ConfigEvent::Paste(text)) => {
                    self.handle_filter_paste(&text);
                    last_frame = None;
                    continue;
                }
                Some(ConfigEvent::Mouse(mouse)) => {
                    match self.handle_browser_mouse(stdout, mouse, &last_hitbox, &mut clicks)? {
                        mouse::BrowserMouseStep::None => continue,
                        mouse::BrowserMouseStep::ForceRedraw => {
                            last_frame = None;
                            continue;
                        }
                        mouse::BrowserMouseStep::Key(mapped) => mapped,
                    }
                }
                Some(ConfigEvent::Key(key)) => key.code,
            };
            match key {
                key if self.filter_mode => self.handle_filter_key(key),
                // 1. 【供应商配置】【两段式Esc】保留过滤词时首次按 Esc 先清空过滤条件，第二次按 Esc 再退出页面
                KeyCode::Esc if !self.filter.is_empty() => {
                    self.clear_retained_filter();
                }
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Left | KeyCode::Char('h') => self.move_left(),
                KeyCode::Right | KeyCode::Char('l') => self.move_right(),
                KeyCode::Up | KeyCode::Char('k') => self.move_up(),
                KeyCode::Down | KeyCode::Char('j') => self.move_down(),
                KeyCode::Char('/') => self.enter_filter_mode(),
                KeyCode::Char('r') => {
                    self.model_cache.remove(&self.catalog_key());
                    self.refresh_models();
                }
                KeyCode::Char('a') if self.active_col == 2 => self.add_custom_model(stdout)?,
                KeyCode::Char('a') => self.add_provider(stdout)?,
                KeyCode::Char('d') if self.active_col == 2 => self.delete_model(stdout)?,
                KeyCode::Char('d') => self.delete_provider(stdout)?,
                KeyCode::Tab if self.active_col == 2 => self.toggle_model_activation(),
                KeyCode::Char(' ' | 'u') if self.active_col == 2 => self.set_current_model(),
                KeyCode::Enter | KeyCode::Char('i') => self.select_or_edit(stdout)?,
                _ => {}
            }
            // 2. 【供应商配置】【交互重绘】子表单或动作完成后清空签名以强制刷新画面
            last_frame = None;
        }
    }

    /// 【供应商配置】【清空保留过滤】清除非编辑态下残留的过滤词并重建完整模型列表。
    fn clear_retained_filter(&mut self) {
        self.filter.clear();
        self.filter_cursor = 0;
        self.model_idx = 0;
        self.rebuild_models();
    }

    /// 【供应商配置】【粘贴过滤词】在过滤输入态下接收粘贴内容并插入光标位置。
    fn handle_filter_paste(&mut self, text: &str) {
        if !self.filter_mode {
            return;
        }
        let cleaned = sanitize_paste_single_line(text);
        if cleaned.is_empty() {
            return;
        }
        insert_str_at_char(&mut self.filter, &mut self.filter_cursor, &cleaned);
        self.model_idx = 0;
        self.rebuild_models();
    }

    /// 【供应商配置】【帧签名】拼接渲染依赖的全部状态用于跳过空闲轮询期的无变化重绘。
    pub(super) fn frame_signature(&self) -> String {
        format!(
            "{}|{}|{}|{}|{:?}|{:?}|{}|{}|{}|{}|{}|{}|{}|{}|{:?}",
            self.active_col,
            self.provider_idx,
            self.org_idx,
            self.model_idx,
            self.hovered,
            self.hovered_footer,
            self.filter,
            self.filter_cursor,
            self.filter_mode,
            self.loading,
            self.status,
            self.orgs.len(),
            self.models.len(),
            self.config.providers.len(),
            terminal::size().ok(),
        )
    }

    /// 【供应商配置】【过滤按键处理】支持游标移动、退格与删除、回车保留过滤及 Esc 清空。
    fn handle_filter_key(&mut self, key: KeyCode) {
        let len = self.filter.chars().count();
        self.filter_cursor = self.filter_cursor.min(len);
        match key {
            KeyCode::Esc => {
                self.leave_filter_mode();
                self.filter.clear();
                self.filter_cursor = 0;
            }
            KeyCode::Enter | KeyCode::Tab => {
                self.leave_filter_mode();
                return;
            }
            KeyCode::Left => {
                self.filter_cursor = self.filter_cursor.saturating_sub(1);
                return;
            }
            KeyCode::Right => {
                self.filter_cursor = (self.filter_cursor + 1).min(len);
                return;
            }
            KeyCode::Home => {
                self.filter_cursor = 0;
                return;
            }
            KeyCode::End => {
                self.filter_cursor = len;
                return;
            }
            KeyCode::Up => {
                self.move_up();
                return;
            }
            KeyCode::Down => {
                self.move_down();
                return;
            }
            KeyCode::Backspace => {
                if self.filter_cursor > 0 {
                    remove_char_before(&mut self.filter, &mut self.filter_cursor);
                }
                if self.filter.is_empty() {
                    self.leave_filter_mode();
                }
                self.model_idx = 0;
            }
            KeyCode::Delete => {
                if self.filter_cursor < len {
                    remove_char_at(&mut self.filter, self.filter_cursor);
                    self.model_idx = 0;
                } else {
                    return;
                }
            }
            KeyCode::Char(ch) => {
                insert_char_at(&mut self.filter, &mut self.filter_cursor, ch);
                self.model_idx = 0;
            }
            _ => return,
        }
        self.rebuild_models();
    }

    pub(super) fn move_left(&mut self) {
        self.active_col = self.active_col.saturating_sub(1);
    }

    pub(super) fn move_right(&mut self) {
        self.active_col = (self.active_col + 1).min(2);
    }

    pub(super) fn move_up(&mut self) {
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

    pub(super) fn move_down(&mut self) {
        match self.active_col {
            0 => {
                self.provider_idx = clamp_index(self.provider_idx + 1, self.config.providers.len());
                self.refresh_models();
            }
            1 => {
                self.org_idx = clamp_index(self.org_idx + 1, self.orgs.len());
                self.rebuild_models();
            }
            2 => self.model_idx = clamp_index(self.model_idx + 1, self.models.len()),
            _ => {}
        }
    }
}

pub(super) fn format_status_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[derive(Clone)]
pub(super) struct ModelEntry {
    pub(super) name: String,
    pub(super) full: String,
}

pub(super) fn model_matches_filter(model: &str, filter: &str) -> bool {
    let filter = filter.trim();
    filter.is_empty()
        || model
            .to_ascii_lowercase()
            .contains(&filter.to_ascii_lowercase())
}

pub(super) fn local_provider_models(provider: &ProviderConfig) -> Vec<String> {
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
    pub(super) fn new(name: &str, full: &str) -> Self {
        Self {
            name: name.to_string(),
            full: full.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn model_filter_matches_substring_case_insensitively() {
        assert!(model_matches_filter("minimax-m3", ""));
        assert!(model_matches_filter("minimax-m3", "Mini"));
        assert!(model_matches_filter("openai/gpt-4o", "gpt-4"));
        assert!(!model_matches_filter("minimax-m3", "claude"));
    }

    #[test]
    fn filter_paste_and_retained_escape_clear_behave_consistently() {
        let mut config = AppConfig::default();
        config.providers[0].models = vec!["openai/gpt-4o".into(), "anthropic/claude-3".into()];
        let mut browser = ProviderBrowser::new(&mut config);
        browser.refresh_models();

        browser.enter_filter_mode();
        browser.handle_filter_paste("gpt\n");
        assert_eq!(browser.filter, "gpt");
        assert_eq!(browser.models.len(), 1);

        browser.handle_filter_key(KeyCode::Enter);
        assert!(!browser.filter_mode);
        assert_eq!(browser.filter, "gpt");

        browser.clear_retained_filter();
        assert!(browser.filter.is_empty());
        assert!(browser.models.len() >= 2);
    }
}
