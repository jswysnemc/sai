use crate::config::{parse_compaction_ratio_text, parse_context_chars, CompactionMode};
use crate::i18n::text as t;
use crate::state::CompactionBudgetPolicy;
use anyhow::{bail, Result};
use crossterm::event::KeyCode;

pub(super) const RESERVE_PRESETS: [usize; 4] = [0, 8_000, 50_000, 100_000];

/// 【上下文】【策略编辑】保存尚未提交的策略与键盘交互状态
pub(super) struct Draft {
    pub policy: CompactionBudgetPolicy,
    pub defaults: CompactionBudgetPolicy,
    pub selected: usize,
    pub input: Option<String>,
    pub input_cursor: usize,
    pub input_selected: bool,
    pub error: Option<String>,
    pub reset: bool,
}

/// 【上下文】【策略编辑】面板结束后需要执行的操作
#[derive(Debug, PartialEq)]
pub(super) enum Outcome {
    Save(CompactionBudgetPolicy),
    Reset,
    Cancel,
}

impl Draft {
    /// 【上下文】【策略编辑】创建草稿，policy 为当前策略，defaults 为恢复目标
    /// 返回: 不改变已保存配置的编辑状态
    pub fn new(policy: CompactionBudgetPolicy, defaults: CompactionBudgetPolicy) -> Self {
        Self {
            policy,
            defaults,
            selected: usize::from(policy.mode == CompactionMode::Reserve),
            input: None,
            input_cursor: 0,
            input_selected: false,
            error: None,
            reset: false,
        }
    }

    /// 【上下文】【策略编辑】解析当前输入用于实时预览
    /// 返回: 校验后的草稿策略；非法输入返回错误
    pub fn preview(&self) -> Result<CompactionBudgetPolicy> {
        if self.reset && self.input.is_none() {
            return Ok(self.defaults);
        }
        let mut policy = self.policy;
        if let Some(input) = &self.input {
            if input.trim().is_empty() {
                bail!(t("Enter a value", "请输入数值"));
            }
            if self.selected == 0 {
                policy.ratio = parse_compaction_ratio_text(input)
                    .map_err(|_| anyhow::anyhow!(t("Use 50–99%", "请输入 50–99%")))?;
            } else {
                policy.reserve_tokens = parse_context_chars(input)
                    .map_err(|_| {
                        anyhow::anyhow!(t(
                            "Use a token count, e.g. 50k",
                            "请输入 token 数，例如 50k"
                        ))
                    })?
                    .unwrap_or(0);
            }
        }
        Ok(policy.with_mode(if self.selected == 0 {
            CompactionMode::Ratio
        } else {
            CompactionMode::Reserve
        }))
    }

    /// 【上下文】【策略编辑】处理按键，只在保存或取消时返回操作
    /// 参数: key 为终端按键；返回: 可选结束操作
    pub fn handle(&mut self, key: KeyCode) -> Option<Outcome> {
        self.error = None;
        // 1. 【上下文】【策略编辑】输入时只修改草稿，回车校验，Esc 撤销本次输入
        if self.input.is_some() {
            self.edit_input(key);
            return None;
        }
        // 2. 【上下文】【策略编辑】导航、数值微调与快捷档均只更新内存草稿
        match key {
            KeyCode::Esc | KeyCode::Char('q') => return Some(Outcome::Cancel),
            KeyCode::BackTab | KeyCode::Up | KeyCode::Down | KeyCode::Tab => {
                self.selected = 1 - self.selected;
                self.reset = false;
            }
            KeyCode::Left | KeyCode::Right | KeyCode::Char('h' | 'l') => {
                let increase = matches!(key, KeyCode::Right | KeyCode::Char('l'));
                if self.selected == 0 {
                    let delta = if increase { 1.0 } else { -1.0 };
                    self.policy.ratio =
                        ((self.policy.ratio * 100.0 + delta).clamp(50.0, 99.0)) / 100.0;
                } else {
                    self.policy.reserve_tokens = if increase {
                        RESERVE_PRESETS
                            .iter()
                            .copied()
                            .find(|value| *value > self.policy.reserve_tokens)
                    } else {
                        RESERVE_PRESETS
                            .iter()
                            .rev()
                            .copied()
                            .find(|value| *value < self.policy.reserve_tokens)
                    }
                    .unwrap_or(self.policy.reserve_tokens);
                }
                self.reset = false;
            }
            KeyCode::Char(ch @ '0'..='3') => {
                self.selected = 1;
                self.policy.reserve_tokens = RESERVE_PRESETS[ch as usize - '0' as usize];
                self.reset = false;
            }
            KeyCode::Char('r') => self.restore_defaults(),
            KeyCode::Char('s' | 'S') | KeyCode::Enter => return Some(self.outcome()),
            KeyCode::Char('e' | 'i') => match self.selected {
                0 => self.begin_input(format_percent(self.policy.ratio)),
                1 => self.begin_input(self.policy.reserve_tokens.to_string()),
                _ => {}
            },
            _ => {}
        }
        None
    }

    /// 【上下文】【策略编辑】恢复默认草稿；返回: 无，仍需保存才生效
    fn restore_defaults(&mut self) {
        self.policy = self.defaults;
        self.selected = usize::from(self.defaults.mode == CompactionMode::Reserve);
        self.reset = true;
    }

    /// 【上下文】【策略编辑】返回保存操作，不进行持久化
    /// 参数：无；返回：恢复默认或保存当前预览策略
    fn outcome(&self) -> Outcome {
        if self.reset {
            Outcome::Reset
        } else {
            Outcome::Save(self.preview().unwrap_or(self.policy))
        }
    }
}

/// 【上下文】【数值展示】将比例格式化为百分数，ratio 为 0–1 比例
/// 返回: 保留有效小数的百分数字符串
pub(super) fn format_percent(ratio: f32) -> String {
    format!("{:.2}", ratio * 100.0)
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

/// 【上下文】【数值展示】为 token 数增加千位分隔，value 为整数
/// 返回: 不舍入的数值文本
pub(super) fn format_tokens(value: usize) -> String {
    let digits = value.to_string();
    let mut output = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            output.push(',');
        }
        output.push(digit);
    }
    output
}
