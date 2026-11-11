use crate::config::parse_context_chars;
use crate::i18n::text as t;
use anyhow::{anyhow, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

const PRESETS: [usize; 4] = [200_000, 300_000, 400_000, 500_000];
const CUSTOM_INDEX: usize = PRESETS.len();
const MAXIMUM_INDEX: usize = CUSTOM_INDEX + 1;
pub(super) const OPTION_COUNT: usize = MAXIMUM_INDEX + 1;

/// 【终端】【上下文选择】单个供应商模型的上限草稿与原始容量。
#[derive(Clone, Debug)]
pub(super) struct ContextSelection {
    index: usize,
    input: String,
    capacity: Option<usize>,
    fallback: usize,
    error: Option<String>,
}

impl ContextSelection {
    /// 【终端】【鼠标上下文】参数为候选下标；选择有效选项并清除错误，返回无。
    pub(super) fn select(&mut self, index: usize) {
        if index < OPTION_COUNT {
            self.index = index;
            self.error = None;
        }
    }
    /// 【终端】【上下文选择】按已保存上限定位预设、自定义或最大选项。
    /// 参数: `limit` 为用户上限，`capacity` 为已知容量，`fallback` 为默认容量；返回: 编辑状态
    pub(super) fn new(limit: Option<usize>, capacity: Option<usize>, fallback: usize) -> Self {
        Self {
            index: limit
                .map(|limit| {
                    PRESETS
                        .iter()
                        .position(|value| *value == limit)
                        .unwrap_or(CUSTOM_INDEX)
                })
                .unwrap_or(MAXIMUM_INDEX),
            input: limit.map(format_tokens).unwrap_or_default(),
            capacity,
            fallback,
            error: None,
        }
    }

    /// 【终端】【上下文选择】返回当前选项下标。
    /// 参数: 无；返回: 选中行下标
    pub(super) fn index(&self) -> usize {
        self.index
    }

    /// 【终端】【上下文选择】移动选项并清除上次输入错误。
    /// 参数: `down` 为是否向下移动；返回: 无
    pub(super) fn move_selection(&mut self, down: bool) {
        self.index = if down {
            (self.index + 1).min(OPTION_COUNT - 1)
        } else {
            self.index.saturating_sub(1)
        };
        self.error = None;
    }

    /// 【终端】【上下文选择】处理数值输入、退格和清空，不修改模型过滤文本。
    /// 参数: `key` 为当前按键；返回: 是否已处理
    pub(super) fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.input.clear();
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Delete => self.input.clear(),
            KeyCode::Char(character)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                if !character.is_ascii_digit()
                    && !matches!(character, '.' | 'k' | 'K' | 'm' | 'M' | '_')
                {
                    return true;
                }
                if self.index != CUSTOM_INDEX {
                    self.input.clear();
                }
                if self.input.len() < 32 {
                    self.input.push(character);
                }
            }
            _ => return false,
        }
        self.index = CUSTOM_INDEX;
        self.error = None;
        true
    }

    /// 【终端】【上下文选择】解析待保存值，最大选项不保存覆盖值。
    /// 参数: 无；返回: 正 token 数、最大时为空，或自定义输入错误
    pub(super) fn selected_limit(&self) -> Result<Option<usize>> {
        match self.index {
            MAXIMUM_INDEX => Ok(None),
            CUSTOM_INDEX => parse_context_chars(&self.input)
                .ok()
                .flatten()
                .map(Some)
                .ok_or_else(|| {
                    anyhow!(t(
                        "Enter a positive token count, e.g. 350k",
                        "请输入正数 token 数量，例如 350k",
                    ))
                }),
            index => Ok(Some(PRESETS[index])),
        }
    }

    /// 【终端】【上下文选择】检查输入并保留错误供面板显示。
    /// 参数: 无；返回: 当前选择是否可保存
    pub(super) fn validate(&mut self) -> bool {
        self.error = self.selected_limit().err().map(|error| error.to_string());
        self.error.is_none()
    }

    /// 【终端】【上下文选择】生成预设、自定义输入及最大容量标签。
    /// 参数: `index` 为选项行下标；返回: 标签，超出选项时为空
    pub(super) fn label(&self, index: usize) -> Option<String> {
        match index {
            CUSTOM_INDEX => Some(format!(
                "{}: {}",
                t("Input", "输入"),
                if self.input.is_empty() {
                    "_"
                } else {
                    &self.input
                }
            )),
            MAXIMUM_INDEX => Some(format!(
                "{} ({})",
                t("Maximum", "最大"),
                format_tokens(self.capacity.unwrap_or(self.fallback)),
            )),
            index => PRESETS.get(index).copied().map(format_tokens),
        }
    }

    /// 【终端】【上下文选择】展示实际预算、容量来源和输入错误。
    /// 参数: 无；返回: 一行提示文本
    pub(super) fn status(&self) -> String {
        if let Some(error) = &self.error {
            return error.clone();
        }
        let Ok(limit) = self.selected_limit() else {
            return t(
                "Input tokens, e.g. 350k or 350000",
                "输入 token 数量，例如 350k 或 350000",
            )
            .to_string();
        };
        let maximum = self.capacity.unwrap_or(self.fallback);
        let effective = limit
            .map(|limit| self.capacity.map_or(limit, |capacity| limit.min(capacity)))
            .unwrap_or(maximum);
        format!(
            "{}: {} · {}: {}",
            t("Effective context", "生效上下文"),
            format_tokens(effective),
            if self.capacity.is_some() {
                t("Model capacity", "模型容量")
            } else {
                t("Default capacity", "默认容量")
            },
            format_tokens(maximum),
        )
    }
}

/// 【终端】【上下文选择】格式化 token 数量，保持非整千输入精度。
/// 参数: `tokens` 为 token 数；返回: 简短数量文本
pub(super) fn format_tokens(tokens: usize) -> String {
    if tokens >= 1_000_000 && tokens % 1_000_000 == 0 {
        format!("{}m", tokens / 1_000_000)
    } else if tokens >= 1_000 && tokens % 1_000 == 0 {
        format!("{}k", tokens / 1_000)
    } else {
        tokens.to_string()
    }
}
