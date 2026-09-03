use crate::config::{AgentProfile, AppConfig};
use crate::config_tui::{parse_provider_model_choice, provider_model_choice_values};
use crate::i18n::text as t;
use crate::paths::SaiPaths;
use anyhow::Result;
use crossterm::event::KeyCode;
use std::io;
use unicode_width::UnicodeWidthStr;

use super::render::{truncate, DIM_STYLE, FOCUS_STYLE, RESET};
use super::state::next_index;
use super::{clear_frame, draw_at, read_key};

/// 名称列宽
const NAME_COLUMN_WIDTH: usize = 36;
/// 模型列宽
const MODEL_COLUMN_WIDTH: usize = 44;
/// 帧固定行数：标题 2 + 空行 + 空行 + 底栏
const FRAME_FIXED_ROWS: usize = 5;

/// 【CLI/TUI】【模型选择】在模型选择器锚点区运行子智能体模型设置。
///
/// 外层循环列出全部 Agent 档案：Enter 进入该档案的模型选择，
/// 选定后立即写回并落盘，Esc 返回上一层。与主选择器共用同一
/// 预留绘制区，帧高一致，退出后由主选择器重绘覆盖。
///
/// 参数:
/// - `stdout`: 终端标准输出
/// - `anchor_y`: 绘制首行行号
/// - `frame_rows`: 预留总行数
/// - `config`: 应用配置
/// - `paths`: Sai 路径集合
///
/// 返回:
/// - 用户取消或已保存时为 Ok
pub(super) fn run(
    stdout: &mut io::Stdout,
    anchor_y: u16,
    frame_rows: u16,
    config: &mut AppConfig,
    paths: &SaiPaths,
) -> Result<()> {
    let mut focus_id: Option<String> = None;
    loop {
        let mut state = SubagentListState::new(config, focus_id.as_deref());
        let Some(profile) = run_list_loop(stdout, anchor_y, frame_rows, &mut state)? else {
            return Ok(());
        };
        focus_id = Some(profile.id.clone());
        let Some(value) = run_model_loop(stdout, anchor_y, frame_rows, config, &profile)? else {
            continue;
        };
        let (provider_id, model) = parse_provider_model_choice(&value);
        if config.set_agent_model(&profile.id, &provider_id, &model) {
            config.save(paths)?;
        }
    }
}

/// 子智能体档案列表交互循环。
///
/// 参数:
/// - `stdout`: 终端标准输出
/// - `anchor_y`: 绘制首行行号
/// - `frame_rows`: 预留总行数
/// - `state`: 列表状态
///
/// 返回:
/// - Enter 选中的档案；Esc 返回时为 None
fn run_list_loop(
    stdout: &mut io::Stdout,
    anchor_y: u16,
    frame_rows: u16,
    state: &mut SubagentListState,
) -> Result<Option<AgentProfile>> {
    loop {
        let lines = render_list(state, frame_rows);
        draw_at(stdout, anchor_y, frame_rows, &lines)?;
        match read_key()?.code {
            KeyCode::Esc => {
                clear_frame(stdout, anchor_y, frame_rows)?;
                return Ok(None);
            }
            KeyCode::Enter => {
                if let Some(profile) = state.selected().cloned() {
                    return Ok(Some(profile));
                }
            }
            KeyCode::Up => state.move_up(),
            KeyCode::Down => state.move_down(),
            _ => {}
        }
    }
}

/// 单个档案的模型选择循环。
///
/// 参数:
/// - `stdout`: 终端标准输出
/// - `anchor_y`: 绘制首行行号
/// - `frame_rows`: 预留总行数
/// - `config`: 应用配置
/// - `profile`: 待设置的档案
///
/// 返回:
/// - Enter 选中的候选值（空串表示继承当前模型）；Esc 返回时为 None
fn run_model_loop(
    stdout: &mut io::Stdout,
    anchor_y: u16,
    frame_rows: u16,
    config: &AppConfig,
    profile: &AgentProfile,
) -> Result<Option<String>> {
    let mut state =
        ModelChoiceState::new(model_entries(config), &profile_choice_value(profile));
    loop {
        let lines = render_chooser(&state, frame_rows, profile);
        draw_at(stdout, anchor_y, frame_rows, &lines)?;
        match read_key()?.code {
            KeyCode::Esc => return Ok(None),
            KeyCode::Enter => {
                if let Some(value) = state.selected_value().cloned() {
                    return Ok(Some(value));
                }
            }
            KeyCode::Up => state.move_up(),
            KeyCode::Down => state.move_down(),
            _ => {}
        }
    }
}

/// 子智能体档案列表状态。
#[derive(Clone, Debug)]
pub(super) struct SubagentListState {
    /// 已解析的全部 Agent 档案
    profiles: Vec<AgentProfile>,
    /// 当前选中下标
    index: usize,
}

impl SubagentListState {
    /// 创建列表状态，并尽量定位到上次编辑的档案。
    ///
    /// 参数:
    /// - `config`: 应用配置
    /// - `focus_id`: 上次编辑的档案标识
    ///
    /// 返回:
    /// - 已定位的列表状态
    pub(super) fn new(config: &AppConfig, focus_id: Option<&str>) -> Self {
        let profiles = config.resolved_agent_profiles();
        let index = focus_id
            .and_then(|id| profiles.iter().position(|profile| profile.id == id))
            .unwrap_or(0);
        Self { profiles, index }
    }

    /// 上移一项，顶端收敛。
    pub(super) fn move_up(&mut self) {
        self.index = self.index.saturating_sub(1);
    }

    /// 下移一项，末端收敛。
    pub(super) fn move_down(&mut self) {
        self.index = next_index(self.index, self.profiles.len());
    }

    /// 返回当前选中档案。
    ///
    /// 返回:
    /// - 选中档案
    pub(super) fn selected(&self) -> Option<&AgentProfile> {
        self.profiles.get(self.index)
    }

    /// 返回当前下标。
    ///
    /// 返回:
    /// - 选中下标
    pub(super) fn index(&self) -> usize {
        self.index
    }

    /// 返回当前选中项附近的档案窗口。
    ///
    /// 参数:
    /// - `max_rows`: 窗口最大行数
    ///
    /// 返回:
    /// - 窗口起始下标与档案切片
    pub(super) fn window(&self, max_rows: usize) -> (usize, &[AgentProfile]) {
        window_of(&self.profiles, self.index, max_rows)
    }
}

/// 模型候选状态。
#[derive(Clone, Debug)]
pub(super) struct ModelChoiceState {
    /// 候选值与展示文本，值格式为 `provider\tmodel`，空值表示继承
    entries: Vec<(String, String)>,
    /// 当前选中下标
    index: usize,
}

impl ModelChoiceState {
    /// 创建候选状态，并定位到档案当前覆盖值。
    ///
    /// 参数:
    /// - `entries`: 候选值与展示文本
    /// - `current_value`: 档案当前覆盖值
    ///
    /// 返回:
    /// - 已定位的候选状态
    pub(super) fn new(entries: Vec<(String, String)>, current_value: &str) -> Self {
        let index = entries
            .iter()
            .position(|(value, _)| value == current_value)
            .unwrap_or(0);
        Self { entries, index }
    }

    /// 上移一项，顶端收敛。
    pub(super) fn move_up(&mut self) {
        self.index = self.index.saturating_sub(1);
    }

    /// 下移一项，末端收敛。
    pub(super) fn move_down(&mut self) {
        self.index = next_index(self.index, self.entries.len());
    }

    /// 返回当前选中下标。
    ///
    /// 返回:
    /// - 选中下标
    pub(super) fn index(&self) -> usize {
        self.index
    }

    /// 返回当前选中候选值。
    ///
    /// 返回:
    /// - 候选值；空列表时为 None
    pub(super) fn selected_value(&self) -> Option<&String> {
        self.entries.get(self.index).map(|(value, _)| value)
    }

    /// 返回当前选中项附近的候选窗口。
    ///
    /// 参数:
    /// - `max_rows`: 窗口最大行数
    ///
    /// 返回:
    /// - 窗口起始下标与候选切片
    pub(super) fn window(&self, max_rows: usize) -> (usize, &[(String, String)]) {
        window_of(&self.entries, self.index, max_rows)
    }
}

/// 计算选中项附近的展示窗口。
///
/// 参数:
/// - `items`: 全部条目
/// - `index`: 选中下标
/// - `max_rows`: 窗口最大行数
///
/// 返回:
/// - 窗口起始下标与条目切片
fn window_of<T>(items: &[T], index: usize, max_rows: usize) -> (usize, &[T]) {
    if max_rows == 0 || items.is_empty() {
        return (0, &[]);
    }
    if items.len() <= max_rows {
        return (0, items);
    }
    let preferred_start = index.saturating_sub(max_rows / 2);
    let start = preferred_start.min(items.len() - max_rows);
    (start, &items[start..start + max_rows])
}

/// 构造模型候选，首项为「继承当前模型」空选项。
///
/// 与 `sai config` 的 Agent 基本信息表单共用同一份候选来源，
/// 保证两处界面可选集合一致。
///
/// 参数:
/// - `config`: 应用配置
///
/// 返回:
/// - 候选值与展示文本列表
fn model_entries(config: &AppConfig) -> Vec<(String, String)> {
    provider_model_choice_values(config, false)
        .into_iter()
        .map(|value| {
            let label = entry_label(config, &value);
            (value, label)
        })
        .collect()
}

/// 返回候选展示文本。
///
/// 参数:
/// - `config`: 应用配置
/// - `value`: 候选值
///
/// 返回:
/// - 空值为继承提示，其余为 `供应商 / 模型`
fn entry_label(config: &AppConfig, value: &str) -> String {
    if value.is_empty() {
        return t("Inherit current model", "继承当前模型").to_string();
    }
    let (provider_id, model) = parse_provider_model_choice(value);
    let provider_name = config
        .providers
        .iter()
        .find(|provider| provider.id == provider_id)
        .map(|provider| provider.display_name.trim())
        .filter(|name| !name.is_empty())
        .unwrap_or(&provider_id);
    format!("{provider_name} / {model}")
}

/// 返回档案当前覆盖值，空串表示继承当前模型。
///
/// 与 `sai config` 的 Agent 表单一致：供应商与模型任一为空都视为未固定。
///
/// 参数:
/// - `profile`: Agent 档案
///
/// 返回:
/// - `provider\tmodel` 覆盖值
fn profile_choice_value(profile: &AgentProfile) -> String {
    if profile.provider_id.trim().is_empty() || profile.model.trim().is_empty() {
        String::new()
    } else {
        format!("{}\t{}", profile.provider_id, profile.model)
    }
}

/// 返回档案模型展示文本。
///
/// 参数:
/// - `profile`: Agent 档案
///
/// 返回:
/// - 未固定时为继承提示，其余为 `供应商 / 模型`
fn profile_model_label(profile: &AgentProfile) -> String {
    if profile.provider_id.trim().is_empty() {
        t("inherit current model", "沿用当前模型").to_string()
    } else if profile.model.trim().is_empty() {
        profile.provider_id.clone()
    } else {
        format!("{} / {}", profile.provider_id, profile.model)
    }
}

/// 渲染档案列表帧。
///
/// 参数:
/// - `state`: 列表状态
/// - `frame_rows`: 预留总行数
///
/// 返回:
/// - 逐行 ANSI 文本
fn render_list(state: &SubagentListState, frame_rows: u16) -> Vec<String> {
    let content_rows = (frame_rows as usize)
        .saturating_sub(FRAME_FIXED_ROWS)
        .max(1);
    let mut lines = vec![
        format!(
            "{FOCUS_STYLE}{}{RESET}",
            t("Subagent models", "子智能体模型")
        ),
        format!(
            "{DIM_STYLE}{}{RESET}",
            t(
                "Set a fixed model per agent; empty inherits the current selection",
                "为各智能体固定模型，留空沿用当前选择",
            )
        ),
        String::new(),
    ];
    let (start, window) = state.window(content_rows);
    for row in 0..content_rows {
        lines.push(list_row_line(state, window, start, row));
    }
    lines.push(String::new());
    lines.push(footer_line(
        t("↑/↓ choose · Enter edit · Esc back", "↑/↓ 选择 · Enter 编辑 · Esc 返回"),
    ));
    lines
}

/// 渲染模型候选帧。
///
/// 参数:
/// - `state`: 候选状态
/// - `frame_rows`: 预留总行数
/// - `profile`: 待设置的档案
///
/// 返回:
/// - 逐行 ANSI 文本
fn render_chooser(
    state: &ModelChoiceState,
    frame_rows: u16,
    profile: &AgentProfile,
) -> Vec<String> {
    let content_rows = (frame_rows as usize)
        .saturating_sub(FRAME_FIXED_ROWS)
        .max(1);
    let mut lines = vec![
        format!(
            "{FOCUS_STYLE}{}{RESET}",
            t("Agent model", "智能体模型")
        ),
        format!(
            "{DIM_STYLE}{} [{}] · {}: {}{RESET}",
            profile.name,
            profile.id,
            t("current", "当前"),
            profile_model_label(profile),
        ),
        String::new(),
    ];
    let (start, window) = state.window(content_rows);
    for row in 0..content_rows {
        lines.push(chooser_row_line(state, window, start, row));
    }
    lines.push(String::new());
    lines.push(footer_line(
        t("↑/↓ choose · Enter save · Esc back", "↑/↓ 选择 · Enter 保存 · Esc 返回"),
    ));
    lines
}

/// 渲染档案列表一行。
///
/// 参数:
/// - `state`: 列表状态
/// - `window`: 当前档案窗口
/// - `start`: 窗口起始下标
/// - `row`: 窗口内行下标
///
/// 返回:
/// - 定宽内容行
fn list_row_line(
    state: &SubagentListState,
    window: &[AgentProfile],
    start: usize,
    row: usize,
) -> String {
    let selected = start + row == state.index();
    let Some(profile) = window.get(row) else {
        return " ".repeat(NAME_COLUMN_WIDTH + MODEL_COLUMN_WIDTH);
    };
    let name = format!("{} [{}]", profile.name, profile.id);
    let model = profile_model_label(profile);
    format!(
        "{}  {}",
        cell(&name, selected, NAME_COLUMN_WIDTH),
        cell(&model, selected, MODEL_COLUMN_WIDTH),
    )
}

/// 渲染模型候选一行。
///
/// 参数:
/// - `state`: 候选状态
/// - `window`: 当前候选窗口
/// - `start`: 窗口起始下标
/// - `row`: 窗口内行下标
///
/// 返回:
/// - 定宽内容行
fn chooser_row_line(
    state: &ModelChoiceState,
    window: &[(String, String)],
    start: usize,
    row: usize,
) -> String {
    let selected = start + row == state.index();
    let Some((_, label)) = window.get(row) else {
        return " ".repeat(NAME_COLUMN_WIDTH + MODEL_COLUMN_WIDTH);
    };
    cell(label, selected, NAME_COLUMN_WIDTH + MODEL_COLUMN_WIDTH)
}

/// 渲染单个候选项。
///
/// 参数:
/// - `label`: 候选项文本
/// - `selected`: 是否为当前选中项
/// - `width`: 列宽
///
/// 返回:
/// - 定宽候选项
fn cell(label: &str, selected: bool, width: usize) -> String {
    let marker = if selected { "› " } else { "  " };
    let style = if selected { FOCUS_STYLE } else { DIM_STYLE };
    let text = truncate(&format!("{marker}{label}"), width);
    let padding = width.saturating_sub(UnicodeWidthStr::width(text.as_str()));
    format!("{style}{text}{RESET}{}", " ".repeat(padding))
}

/// 渲染底栏操作提示。
///
/// 参数:
/// - `text`: 提示文本
///
/// 返回:
/// - 提示行
fn footer_line(text: &str) -> String {
    format!("{DIM_STYLE}{text}{RESET}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一个带模型覆盖的档案。
    fn profile(id: &str, provider_id: &str, model: &str) -> AgentProfile {
        AgentProfile {
            id: id.to_string(),
            name: id.to_string(),
            provider_id: provider_id.to_string(),
            model: model.to_string(),
            ..AgentProfile::default()
        }
    }

    /// 构造带一个可用供应商模型的配置。
    fn config_with_models() -> AppConfig {
        let mut config = AppConfig::default();
        config.providers[0].models = vec!["model-a".to_string()];
        config.providers[0].default_model = "model-a".to_string();
        config
    }

    /// 【CLI】【模型选择】验证列表定位到上次编辑的档案。
    #[test]
    fn list_state_focuses_the_previously_edited_agent() {
        let mut config = AppConfig::default();
        config.agents.push(profile("pinned", "openai", "gpt-5"));

        let state = SubagentListState::new(&config, Some("pinned"));

        assert_eq!(state.selected().unwrap().id, "pinned");
        let state = SubagentListState::new(&config, Some("missing"));
        assert_eq!(state.index(), 0, "未知档案应回退到首项");
    }

    /// 【CLI】【模型选择】验证列表包含全部内置档案且移动在两端收敛。
    #[test]
    fn list_moves_clamp_at_both_ends() {
        let config = AppConfig::default();
        let len = config.resolved_agent_profiles().len();
        let mut state = SubagentListState::new(&config, None);
        assert!(len >= 5, "内置档案应全部出现在列表里");

        state.move_up();
        state.move_up();
        assert_eq!(state.index(), 0);

        for _ in 0..len + 3 {
            state.move_down();
        }
        assert_eq!(state.index(), len - 1, "下移不应越过末项");
    }

    /// 【CLI】【模型选择】验证档案窗口始终包含当前选中项。
    #[test]
    fn list_window_tracks_the_selection() {
        let mut config = AppConfig::default();
        for index in 0..30 {
            config.agents.push(profile(&format!("agent-{index}"), "", ""));
        }
        let mut state = SubagentListState::new(&config, Some("agent-25"));

        for _ in 0..4 {
            state.move_down();
        }
        let (start, window) = state.window(8);
        assert!(start <= state.index());
        assert!(start + window.len() > state.index());
        assert_eq!(window.len(), 8);
    }

    /// 【CLI】【模型选择】验证候选以继承空选项开头并覆盖供应商模型。
    #[test]
    fn model_entries_lead_with_the_inherit_choice() {
        let config = config_with_models();
        let provider_id = config.providers[0].id.clone();
        let display_name = config.providers[0].display_name.clone();

        let entries = model_entries(&config);

        assert_eq!(entries[0].0, "", "首项必须是继承空选项");
        assert!(entries[0].1.contains(t("Inherit", "继承")));
        let values = entries.iter().map(|(value, _)| value.clone()).collect::<Vec<_>>();
        assert!(values
            .iter()
            .any(|value| value == &format!("{provider_id}\tmodel-a")));
        let labels = entries.iter().map(|(_, label)| label.clone()).collect::<Vec<_>>();
        assert!(labels
            .iter()
            .any(|label| label.ends_with("/ model-a")), "候选应展示模型名: {labels:?}");
        if !display_name.trim().is_empty() {
            assert!(labels
                .iter()
                .any(|label| label.contains(display_name.trim())));
        }
    }

    /// 【CLI】【模型选择】验证候选定位到档案当前覆盖值。
    #[test]
    fn model_state_positions_at_the_current_override() {
        let entries = vec![
            (String::new(), "inherit".to_string()),
            ("openai\tgpt-5".to_string(), "GPT / gpt-5".to_string()),
        ];

        let state = ModelChoiceState::new(entries.clone(), "openai\tgpt-5");
        assert_eq!(state.selected_value().unwrap(), "openai\tgpt-5");

        let state = ModelChoiceState::new(entries, "unknown");
        assert_eq!(state.selected_value().unwrap(), "", "未知值应回退到继承项");
    }

    /// 【CLI】【模型选择】验证候选移动在两端收敛。
    #[test]
    fn model_moves_clamp_at_both_ends() {
        let entries = vec![
            (String::new(), "inherit".to_string()),
            ("a\tm1".to_string(), "A / m1".to_string()),
            ("b\tm2".to_string(), "B / m2".to_string()),
        ];
        let mut state = ModelChoiceState::new(entries, "b\tm2");

        state.move_down();
        state.move_down();
        assert_eq!(state.index(), 2);
        state.move_up();
        state.move_up();
        state.move_up();
        assert_eq!(state.index(), 0);
    }

    /// 【CLI】【模型选择】验证为内置档案固定模型会物化完整档案。
    ///
    /// 只写 id 与模型的空档案会让探索 Agent 退化为全量工具，
    /// 因此物化必须保留工具白名单等内置能力。
    #[test]
    fn setting_a_builtin_agent_model_preserves_its_capabilities() {
        let mut config = AppConfig::default();
        let provider_id = config.providers[0].id.clone();

        assert!(config.set_agent_model("explore", &provider_id, "model-a"));

        let stored = config
            .agents
            .iter()
            .find(|profile| profile.id == "explore")
            .expect("内置档案应物化到配置");
        assert_eq!(stored.provider_id, provider_id);
        assert_eq!(stored.model, "model-a");
        assert!(!stored.enabled_tools.is_empty(), "探索工具白名单必须保留");
        let resolved = config
            .resolved_agent_profiles()
            .into_iter()
            .find(|profile| profile.id == "explore")
            .unwrap();
        assert_eq!(resolved.model, "model-a");
    }

    /// 【CLI】【模型选择】验证再次设置覆盖会原位更新而不重复入档。
    #[test]
    fn setting_an_agent_model_again_updates_in_place() {
        let mut config = AppConfig::default();
        let provider_id = config.providers[0].id.clone();
        config.set_agent_model("explore", &provider_id, "model-a");

        assert!(config.set_agent_model("explore", &provider_id, "model-b"));
        let matches = config
            .agents
            .iter()
            .filter(|profile| profile.id == "explore")
            .count();
        assert_eq!(matches, 1, "同档案不应重复物化");
        let stored = config
            .agents
            .iter()
            .find(|profile| profile.id == "explore")
            .unwrap();
        assert_eq!(stored.model, "model-b");

        assert!(!config.set_agent_model("explore", &provider_id, "model-b"), "无变化不应报告改动");
    }

    /// 【CLI】【模型选择】验证继承空选项会清除既有覆盖。
    #[test]
    fn the_inherit_choice_clears_an_existing_override() {
        let mut config = AppConfig::default();
        let provider_id = config.providers[0].id.clone();
        config.set_agent_model("explore", &provider_id, "model-a");

        assert!(config.set_agent_model("explore", "", ""));
        let stored = config
            .agents
            .iter()
            .find(|profile| profile.id == "explore")
            .unwrap();
        assert!(stored.provider_id.is_empty());
        assert!(stored.model.is_empty());
    }

    /// 【CLI】【模型选择】验证对未配置档案选择继承是无操作。
    #[test]
    fn the_inherit_choice_is_a_noop_for_untouched_builtins() {
        let mut config = AppConfig::default();

        assert!(!config.set_agent_model("explore", "", ""));
        assert!(
            !config.agents.iter().any(|profile| profile.id == "explore"),
            "不应为无覆盖可清的内置档案物化配置"
        );
    }

    /// 【CLI】【模型选择】验证旧版迁移档案的覆盖也能被继承清除。
    #[test]
    fn the_inherit_choice_clears_a_legacy_override() {
        let mut config = AppConfig::default();
        config.subagent.profiles = vec![crate::config::SubagentProfile {
            id: "explore".to_string(),
            name: "旧探索".to_string(),
            description: String::new(),
            system_prompt: String::new(),
            provider_id: "legacy-provider".to_string(),
            model: "legacy-model".to_string(),
            thinking_level: "auto".to_string(),
            exposed: true,
        }];

        assert!(config.set_agent_model("explore", "", ""));
        let resolved = config
            .resolved_agent_profiles()
            .into_iter()
            .find(|profile| profile.id == "explore")
            .unwrap();
        assert!(resolved.provider_id.is_empty(), "旧覆盖应被统一档案盖掉");
        assert!(resolved.model.is_empty());
    }

    /// 【CLI】【模型选择】验证未知档案返回无改动。
    #[test]
    fn unknown_agents_report_no_change() {
        let mut config = AppConfig::default();

        assert!(!config.set_agent_model("missing", "openai", "gpt-5"));
        assert!(config.agents.is_empty());
    }

    /// 【CLI】【模型选择】验证档案覆盖值与展示文本的换算。
    #[test]
    fn profile_values_and_labels_agree() {
        let mut pinned = profile("pinned", "openai", "gpt-5");
        assert_eq!(profile_choice_value(&pinned), "openai\tgpt-5");
        assert_eq!(profile_model_label(&pinned), "openai / gpt-5");

        pinned.model = String::new();
        assert_eq!(profile_choice_value(&pinned), "", "部分覆盖视为继承");
        assert_eq!(profile_model_label(&pinned), "openai");

        let inherit = profile("inherit", "", "");
        assert_eq!(profile_choice_value(&inherit), "");
        assert!(profile_model_label(&inherit).contains(t("inherit", "沿用")));
    }
}
