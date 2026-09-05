use super::super::state::{next_index, PickerState};
use crate::config::{AppConfig, ProviderModelChoice, SubagentModelChoice};
use crate::i18n::text as t;

/// 子任务设置列表中的共享默认值或单个任务类型。
#[derive(Clone, Debug)]
pub(super) struct SubagentTarget {
    pub profile_id: Option<String>,
    pub name: String,
    pub selection: SubagentModelChoice,
}

/// 子任务设置列表状态，始终包含共享默认值这一项。
pub(super) struct SubagentListState {
    pub targets: Vec<SubagentTarget>,
    pub index: usize,
}

impl SubagentListState {
    /// 读取子任务设置并恢复选中项；参数为配置和上次类型标识，返回列表状态。
    pub(super) fn new(config: &AppConfig, focus_id: Option<&str>) -> Self {
        let settings = config.subagent_model_settings();
        let mut targets = vec![SubagentTarget {
            profile_id: None,
            name: t("Shared defaults", "共享默认值").to_string(),
            selection: settings.defaults,
        }];
        targets.extend(settings.profiles.into_iter().map(|profile| SubagentTarget {
            profile_id: Some(profile.id),
            name: profile.name,
            selection: profile.selection,
        }));
        let index = targets
            .iter()
            .position(|target| target.profile_id.as_deref() == focus_id)
            .unwrap_or(0);
        Self { targets, index }
    }

    /// 返回当前编辑对象，无参数，返回始终存在的选中项。
    pub(super) fn selected(&self) -> &SubagentTarget {
        &self.targets[self.index]
    }

    /// 上移选中项，无参数和返回值。
    pub(super) fn move_up(&mut self) {
        self.index = self.index.saturating_sub(1);
    }

    /// 下移选中项，无参数和返回值。
    pub(super) fn move_down(&mut self) {
        self.index = next_index(self.index, self.targets.len());
    }
}

/// 【终端】【子任务模型】创建包含继承项的双列模型、思考选择状态。
///
/// 参数: `config` 为候选来源，`target` 为编辑目标
/// 返回: 定位到当前模型与思考等级的选择器
pub(super) fn choice_state(config: &AppConfig, target: &SubagentTarget) -> PickerState {
    let mut choices = config.provider_model_choices();
    let current = &target.selection;
    if !current.provider_id.is_empty()
        && !current.model.is_empty()
        && !choices.iter().any(|choice| {
            choice.provider_id == current.provider_id && choice.model == current.model
        })
    {
        choices.insert(
            0,
            ProviderModelChoice {
                provider_id: current.provider_id.clone(),
                provider_name: current.provider_id.clone(),
                model: current.model.clone(),
            },
        );
    }
    choices.insert(
        0,
        ProviderModelChoice {
            provider_id: String::new(),
            model: String::new(),
            provider_name: if target.profile_id.is_none() {
                t("Inherit conversation model", "沿用主对话模型")
            } else {
                t("Inherit shared defaults", "沿用共享默认值")
            }
            .to_string(),
        },
    );
    PickerState::new(
        choices,
        super::super::THINKING_LEVELS.to_vec(),
        &current.provider_id,
        &current.model,
        &current.thinking_level,
    )
}
