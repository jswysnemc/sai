//! 配置表单字段模型定义：支持动态双语标签、布尔开关、枚举选项、多行文本、掩码密钥与静态只读字段。

use crate::i18n::text as t;

/// 【配置表单】【字段模型】保存单个表单字段、只读展示项或视觉分组标题。
pub(crate) struct Field {
    pub(crate) label: String,
    pub(crate) value: String,
    pub(crate) textarea: bool,
    pub(crate) boolean: bool,
    pub(crate) secret: bool,
    pub(crate) readonly: bool,
    /// 分组标题行：不可选中、不可编辑，仅用于视觉分区
    pub(crate) section: bool,
    pub(crate) choices: Vec<String>,
    pub(crate) empty_choice_label: &'static str,
}

impl Field {
    /// 【配置表单】【文本字段】创建普通单行可编辑文本字段。
    ///
    /// 参数:
    /// - `label`: 字段显示标签
    /// - `value`: 初始文本值
    ///
    /// 返回:
    /// - 可编辑文本字段
    pub(crate) fn new(label: impl Into<String>, value: String) -> Self {
        Self {
            label: label.into(),
            value,
            textarea: false,
            boolean: false,
            secret: false,
            readonly: false,
            section: false,
            choices: Vec::new(),
            empty_choice_label: t("Use current Provider", "使用当前 Provider"),
        }
    }

    /// 【配置表单】【分组标题】创建导航自动跳过的分组标题行。
    ///
    /// 参数:
    /// - `label`: 分组标题文本
    ///
    /// 返回:
    /// - 不可编辑的分组标题字段
    pub(crate) fn section(label: impl Into<String>) -> Self {
        let mut field = Self::new(label, String::new());
        field.section = true;
        field
    }

    /// 【配置表单】【开关字段】创建行内切换的布尔开关字段。
    ///
    /// 参数:
    /// - `label`: 字段显示标签
    /// - `value`: 初始布尔状态
    ///
    /// 返回:
    /// - 布尔开关字段
    pub(crate) fn boolean(label: impl Into<String>, value: bool) -> Self {
        Self {
            label: label.into(),
            value: value.to_string(),
            textarea: false,
            boolean: true,
            secret: false,
            readonly: false,
            section: false,
            choices: Vec::new(),
            empty_choice_label: t("Use current Provider", "使用当前 Provider"),
        }
    }

    /// 【配置表单】【多行字段】创建调用外部 `$EDITOR` 编辑的多行文本字段。
    ///
    /// 参数:
    /// - `label`: 字段显示标签
    /// - `value`: 初始多行文本
    ///
    /// 返回:
    /// - 多行文本字段
    pub(crate) fn textarea(label: impl Into<String>, value: String) -> Self {
        Self {
            label: label.into(),
            value,
            textarea: true,
            boolean: false,
            secret: false,
            readonly: false,
            section: false,
            choices: Vec::new(),
            empty_choice_label: t("Use current Provider", "使用当前 Provider"),
        }
    }

    /// 【配置表单】【秘密字段】启用默认定宽星号掩码显示。
    ///
    /// 参数:
    /// - `self`: 原始字段
    ///
    /// 返回:
    /// - 标记为秘密的字段
    pub(crate) fn secret(mut self) -> Self {
        self.secret = true;
        self
    }

    /// 【配置表单】【只读字段】标记字段为静态只读展示，禁止编辑且不显示枚举切换箭头。
    ///
    /// 参数:
    /// - `self`: 原始字段
    ///
    /// 返回:
    /// - 标记为只读的字段
    pub(crate) fn readonly(mut self) -> Self {
        self.readonly = true;
        self
    }

    /// 【配置表单】【静态选项】为字段绑定静态切片候选项。
    ///
    /// 参数:
    /// - `choices`: 候选值切片
    ///
    /// 返回:
    /// - 带候选值的字段
    pub(crate) fn choices(mut self, choices: &[&str]) -> Self {
        self.choices = choices.iter().map(|item| item.to_string()).collect();
        self
    }

    /// 【配置表单】【动态选项】为字段绑定动态生成的候选项列表。
    ///
    /// 参数:
    /// - `choices`: 候选值向量
    ///
    /// 返回:
    /// - 带候选值的字段
    pub(crate) fn choices_owned(mut self, choices: Vec<String>) -> Self {
        self.choices = choices;
        self
    }

    /// 【配置表单】【空选项说明】自定义未选择（空字符串）时的回退展示标签。
    ///
    /// 参数:
    /// - `label`: 空值展示文案
    ///
    /// 返回:
    /// - 更新空值说明后的字段
    pub(crate) fn empty_choice_label(mut self, label: &'static str) -> Self {
        self.empty_choice_label = label;
        self
    }
}
