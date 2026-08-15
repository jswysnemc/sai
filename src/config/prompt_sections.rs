use serde::{Deserialize, Serialize};

/// 返回布尔默认真值。
///
/// 参数:
/// - 无
///
/// 返回:
/// - 恒为 true
fn default_true() -> bool {
    true
}

/// 系统提示词各内置分段的开关。
///
/// 分段拆出来是为了能配出真正空白的 Agent：这些内容原本硬拼在提示词
/// 组装函数里，配置界面上看不见也关不掉，"0 提示词"于是无法表达。
/// 全部默认开启，旧配置行为不变。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptSectionToggles {
    /// 内置默认人设；关闭后 Agent 自己的 system_prompt 为空即真的空白
    #[serde(default = "default_true")]
    pub builtin_persona: bool,
    /// 当前用户身份档案
    #[serde(default = "default_true")]
    pub user_identity: bool,
    /// 可用 skills 目录
    #[serde(default = "default_true")]
    pub skills_catalog: bool,
    /// 运行时状态覆盖契约
    #[serde(default = "default_true")]
    pub state_contract: bool,
    /// 记忆使用契约
    #[serde(default = "default_true")]
    pub memory_contract: bool,
    /// 当前运行模式的约束说明
    #[serde(default = "default_true")]
    pub mode_reminder: bool,
}

impl Default for PromptSectionToggles {
    /// 返回全部分段启用的默认值。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 全开的分段开关
    fn default() -> Self {
        Self {
            builtin_persona: true,
            user_identity: true,
            skills_catalog: true,
            state_contract: true,
            memory_contract: true,
            mode_reminder: true,
        }
    }
}

impl PromptSectionToggles {
    /// 返回全部分段关闭的开关。
    ///
    /// 目前只有测试构造空白 Agent 时用到。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 全关的分段开关
    #[cfg(test)]
    pub fn all_disabled() -> Self {
        Self {
            builtin_persona: false,
            user_identity: false,
            skills_catalog: false,
            state_contract: false,
            memory_contract: false,
            mode_reminder: false,
        }
    }

}

/// 一个分段的元信息，供配置界面枚举与展示。
pub struct PromptSectionInfo {
    /// 稳定标识，与配置字段名一致
    pub id: &'static str,
    /// 英文标签
    pub label_en: &'static str,
    /// 中文标签
    pub label_zh: &'static str,
    /// 英文说明
    pub hint_en: &'static str,
    /// 中文说明
    pub hint_zh: &'static str,
}

/// 全部内置分段的清单，顺序即它们在提示词中出现的顺序。
pub const PROMPT_SECTIONS: &[PromptSectionInfo] = &[
    PromptSectionInfo {
        id: "builtin_persona",
        label_en: "Built-in persona",
        label_zh: "内置人设",
        hint_en: "Fallback persona used when the agent's own system prompt is empty. Turn off for a truly blank prompt.",
        hint_zh: "Agent 自身提示词为空时使用的兜底人设。要配出真正空白的提示词就关掉它。",
    },
    PromptSectionInfo {
        id: "user_identity",
        label_en: "User identity",
        label_zh: "用户身份",
        hint_en: "The active user profile appended as <current-user-profile>.",
        hint_zh: "以 <current-user-profile> 追加的当前用户档案。",
    },
    PromptSectionInfo {
        id: "skills_catalog",
        label_en: "Skills catalog",
        label_zh: "技能目录",
        hint_en: "List of available skills. Independent of whether skill tools are registered.",
        hint_zh: "可用 skills 清单，与是否注册技能工具相互独立。",
    },
    PromptSectionInfo {
        id: "state_contract",
        label_en: "Runtime state contract",
        label_zh: "运行时状态契约",
        hint_en: "Explains how to read the working directory, time and other state tags. Turning it off may degrade behaviour.",
        hint_zh: "说明如何读取工作目录、时间等状态标签。关掉后模型可能读不懂运行状态。",
    },
    PromptSectionInfo {
        id: "memory_contract",
        label_en: "Memory contract",
        label_zh: "记忆契约",
        hint_en: "Tells the model when to write memories. Only injected when memory is enabled.",
        hint_zh: "告诉模型何时该写记忆，仅在记忆功能启用时注入。",
    },
    PromptSectionInfo {
        id: "mode_reminder",
        label_en: "Mode reminder",
        label_zh: "模式提示词",
        hint_en: "Constraints for the current run mode: YOLO, audited or plan.",
        hint_zh: "当前运行模式的约束说明：YOLO、审计或计划模式。",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证默认全部启用。
    ///
    /// 任何一段默认关闭都会静默改变既有配置的行为。
    #[test]
    fn every_section_defaults_to_enabled() {
        let toggles = PromptSectionToggles::default();

        assert!(toggles.builtin_persona);
        assert!(toggles.user_identity);
        assert!(toggles.skills_catalog);
        assert!(toggles.state_contract);
        assert!(toggles.memory_contract);
        assert!(toggles.mode_reminder);
    }

    /// 验证全关构造确实每一项都关。
    #[test]
    fn the_all_disabled_constructor_turns_everything_off() {
        let toggles = PromptSectionToggles::all_disabled();
        let json = serde_json::to_value(&toggles).unwrap();

        assert!(json.as_object().unwrap().values().all(|value| value == false));
    }

    /// 验证清单覆盖结构体的每一个字段。
    ///
    /// 漏一项就意味着那个分段在配置界面上不可见，等于关不掉。
    #[test]
    fn the_catalog_covers_every_toggle() {
        let json = serde_json::to_value(PromptSectionToggles::default()).unwrap();
        let fields = json.as_object().unwrap();

        assert_eq!(fields.len(), PROMPT_SECTIONS.len());
        for section in PROMPT_SECTIONS {
            assert!(fields.contains_key(section.id), "清单缺少 {}", section.id);
        }
    }

    /// 验证缺失字段时按默认值补全。
    ///
    /// 旧配置里没有这个对象，反序列化必须落到全开而不是全关。
    #[test]
    fn missing_fields_fall_back_to_enabled() {
        let toggles: PromptSectionToggles = serde_json::from_str("{}").unwrap();

        assert_eq!(toggles, PromptSectionToggles::default());
    }

    /// 验证可以只关闭其中一段。
    #[test]
    fn a_single_section_can_be_turned_off() {
        let toggles: PromptSectionToggles =
            serde_json::from_str(r#"{"state_contract": false}"#).unwrap();

        assert!(!toggles.state_contract);
        assert!(toggles.builtin_persona);
    }
}
