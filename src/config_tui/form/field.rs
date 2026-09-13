use crate::i18n::text as t;

/// 【配置表单】【字段模型】保存一个可编辑字段或分组标题
pub(crate) struct Field {
    pub(crate) label: &'static str,
    pub(crate) value: String,
    pub(crate) textarea: bool,
    pub(crate) boolean: bool,
    pub(crate) secret: bool,
    /// 分组标题行：不可选中、不可编辑，仅用于视觉分区
    pub(crate) section: bool,
    pub(crate) choices: Vec<String>,
    pub(crate) empty_choice_label: &'static str,
}

impl Field {
    /// 【配置表单】【文本字段】创建普通单行文本字段
    /// @param label 字段标签；value 为初始文本
    /// @returns 可编辑字段
    pub(crate) fn new(label: &'static str, value: String) -> Self {
        Self {
            label,
            value,
            textarea: false,
            boolean: false,
            secret: false,
            section: false,
            choices: Vec::new(),
            empty_choice_label: t("Use current Provider", "使用当前 Provider"),
        }
    }

    /// 【配置表单】【分组标题】创建导航自动跳过的标题行
    /// @param label 分组标签
    /// @returns 不可编辑的标题字段
    pub(crate) fn section(label: &'static str) -> Self {
        let mut field = Self::new(label, String::new());
        field.section = true;
        field
    }

    /// 【配置表单】【开关字段】创建布尔开关
    /// @param label 字段标签；value 为初始开关状态
    /// @returns 布尔字段
    pub(crate) fn boolean(label: &'static str, value: bool) -> Self {
        Self {
            label,
            value: value.to_string(),
            textarea: false,
            boolean: true,
            secret: false,
            section: false,
            choices: Vec::new(),
            empty_choice_label: t("Use current Provider", "使用当前 Provider"),
        }
    }

    /// 【配置表单】【多行字段】创建使用外部编辑器的文本字段
    /// @param label 字段标签；value 为初始文本
    /// @returns 多行字段
    pub(crate) fn textarea(label: &'static str, value: String) -> Self {
        Self {
            label,
            value,
            textarea: true,
            boolean: false,
            secret: false,
            section: false,
            choices: Vec::new(),
            empty_choice_label: t("Use current Provider", "使用当前 Provider"),
        }
    }

    /// 【配置表单】【秘密字段】启用默认掩码显示
    /// @returns 保留原值并标记为秘密的字段
    pub(crate) fn secret(mut self) -> Self {
        self.secret = true;
        self
    }

    /// 【配置表单】【静态选项】为字段保存候选文本
    /// @param choices 候选值列表
    /// @returns 带候选值的字段
    pub(crate) fn choices(mut self, choices: &[&str]) -> Self {
        self.choices = choices.iter().map(|item| item.to_string()).collect();
        self
    }

    /// 【配置表单】【动态选项】接收已构造的候选文本
    /// @param choices 候选值列表
    /// @returns 带候选值的字段
    pub(crate) fn choices_owned(mut self, choices: Vec<String>) -> Self {
        self.choices = choices;
        self
    }

    /// 【配置表单】【空选项说明】设置未选择时的展示文本
    /// @param label 空选项标签
    /// @returns 更新后的字段
    pub(crate) fn empty_choice_label(mut self, label: &'static str) -> Self {
        self.empty_choice_label = label;
        self
    }
}
