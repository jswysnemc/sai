use crate::config::ProviderModelChoice;

/// 选择器当前聚焦的列。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PickerColumn {
    /// 模型列
    Model,
    /// 思考等级列
    Thinking,
}

/// models 命令的交互选择状态。
#[derive(Clone, Debug)]
pub(super) struct PickerState {
    /// 全部供应商模型，用于重新过滤
    all_models: Vec<ProviderModelChoice>,
    /// 当前过滤后的供应商模型
    models: Vec<ProviderModelChoice>,
    /// 可选思考等级
    levels: Vec<&'static str>,
    filter: String,
    model_index: usize,
    level_index: usize,
    column: PickerColumn,
}

impl PickerState {
    /// 创建选择状态，并定位到当前生效的供应商、模型与思考等级。
    ///
    /// 参数:
    /// - `models`: 全部供应商的可用模型
    /// - `levels`: 可选思考等级
    /// - `current_provider`: 当前生效供应商
    /// - `current_model`: 当前生效模型
    /// - `current_level`: 当前生效思考等级
    ///
    /// 返回:
    /// - 已定位到当前取值的选择状态
    pub(super) fn new(
        models: Vec<ProviderModelChoice>,
        levels: Vec<&'static str>,
        current_provider: &str,
        current_model: &str,
        current_level: &str,
    ) -> Self {
        let model_index = models
            .iter()
            .position(|choice| {
                choice.provider_id == current_provider && choice.model == current_model
            })
            .unwrap_or(0);
        let level_index = levels
            .iter()
            .position(|level| *level == current_level)
            .unwrap_or(0);
        Self {
            all_models: models.clone(),
            models,
            levels,
            filter: String::new(),
            model_index,
            level_index,
            column: PickerColumn::Model,
        }
    }

    /// 在当前列内上移一项。
    ///
    /// 返回:
    /// - 无
    pub(super) fn move_up(&mut self) {
        match self.column {
            PickerColumn::Model => self.model_index = self.model_index.saturating_sub(1),
            PickerColumn::Thinking => self.level_index = self.level_index.saturating_sub(1),
        }
    }

    /// 在当前列内下移一项。
    ///
    /// 返回:
    /// - 无
    pub(super) fn move_down(&mut self) {
        match self.column {
            PickerColumn::Model => {
                self.model_index = next_index(self.model_index, self.models.len());
            }
            PickerColumn::Thinking => {
                self.level_index = next_index(self.level_index, self.levels.len());
            }
        }
    }

    /// 切换到模型列。
    ///
    /// 返回:
    /// - 无
    pub(super) fn focus_model(&mut self) {
        self.column = PickerColumn::Model;
    }

    /// 切换到思考等级列。
    ///
    /// 返回:
    /// - 无
    pub(super) fn focus_thinking(&mut self) {
        self.column = PickerColumn::Thinking;
    }

    /// 追加一个过滤字符并重新筛选全部供应商模型。
    ///
    /// 参数:
    /// - `ch`: 用户输入字符
    ///
    /// 返回:
    /// - 无
    pub(super) fn push_filter(&mut self, ch: char) {
        if ch.is_control() {
            return;
        }
        self.filter.push(ch);
        self.column = PickerColumn::Model;
        self.apply_filter();
    }

    /// 删除过滤文本的最后一个字符。
    ///
    /// 返回:
    /// - 无
    pub(super) fn pop_filter(&mut self) {
        self.filter.pop();
        self.column = PickerColumn::Model;
        self.apply_filter();
    }

    /// 清空过滤文本并恢复全部供应商模型。
    ///
    /// 返回:
    /// - 无
    pub(super) fn clear_filter(&mut self) {
        if self.filter.is_empty() {
            return;
        }
        self.filter.clear();
        self.column = PickerColumn::Model;
        self.apply_filter();
    }

    /// 返回当前聚焦的列。
    ///
    /// 返回:
    /// - 聚焦列
    pub(super) fn column(&self) -> PickerColumn {
        self.column
    }

    /// 返回过滤后的供应商模型列表。
    ///
    /// 返回:
    /// - 供应商模型列表
    pub(super) fn models(&self) -> &[ProviderModelChoice] {
        &self.models
    }

    /// 返回全部供应商模型数量。
    ///
    /// 返回:
    /// - 全部候选数量
    pub(super) fn total_model_count(&self) -> usize {
        self.all_models.len()
    }

    /// 返回当前过滤文本。
    ///
    /// 返回:
    /// - 过滤文本
    pub(super) fn filter(&self) -> &str {
        &self.filter
    }

    /// 返回当前选中项附近的模型窗口。
    ///
    /// 参数:
    /// - `max_rows`: 窗口最大行数
    ///
    /// 返回:
    /// - 窗口起始下标与模型切片
    pub(super) fn model_window(&self, max_rows: usize) -> (usize, &[ProviderModelChoice]) {
        if max_rows == 0 || self.models.is_empty() {
            return (0, &[]);
        }
        if self.models.len() <= max_rows {
            return (0, &self.models);
        }
        let preferred_start = self.model_index.saturating_sub(max_rows / 2);
        let start = preferred_start.min(self.models.len() - max_rows);
        (start, &self.models[start..start + max_rows])
    }

    /// 返回可选思考等级列表。
    ///
    /// 返回:
    /// - 思考等级列表
    pub(super) fn levels(&self) -> &[&'static str] {
        &self.levels
    }

    /// 返回当前模型下标。
    ///
    /// 返回:
    /// - 模型下标
    pub(super) fn model_index(&self) -> usize {
        self.model_index
    }

    /// 返回当前思考等级下标。
    ///
    /// 返回:
    /// - 思考等级下标
    pub(super) fn level_index(&self) -> usize {
        self.level_index
    }

    /// 返回当前选中的供应商模型。
    ///
    /// 返回:
    /// - 供应商模型；无匹配项时为 None
    pub(super) fn selected_model(&self) -> Option<&ProviderModelChoice> {
        self.models.get(self.model_index)
    }

    /// 返回当前选中的思考等级。
    ///
    /// 返回:
    /// - 思考等级
    pub(super) fn selected_level(&self) -> &'static str {
        self.levels.get(self.level_index).copied().unwrap_or("auto")
    }

    /// 根据过滤文本重建模型列表，并尽量保留原选中项。
    ///
    /// 返回:
    /// - 无
    fn apply_filter(&mut self) {
        let previous = self.selected_model().map(ProviderModelChoice::value);
        let needle = self.filter.trim().to_lowercase();
        self.models = if needle.is_empty() {
            self.all_models.clone()
        } else {
            self.all_models
                .iter()
                .filter(|choice| model_matches(choice, &needle))
                .cloned()
                .collect()
        };
        self.model_index = previous
            .as_deref()
            .and_then(|value| {
                self.models
                    .iter()
                    .position(|choice| choice.value() == value)
            })
            .unwrap_or(0);
    }
}

/// 判断供应商模型是否匹配过滤文本。
///
/// 参数:
/// - `choice`: 供应商模型候选
/// - `needle`: 已归一化的小写过滤文本
///
/// 返回:
/// - 供应商标识、展示名或模型名命中时为 true
fn model_matches(choice: &ProviderModelChoice, needle: &str) -> bool {
    choice.provider_id.to_lowercase().contains(needle)
        || choice.provider_name.to_lowercase().contains(needle)
        || choice.model.to_lowercase().contains(needle)
}

/// 计算下移后的下标。
///
/// 参数:
/// - `index`: 当前下标
/// - `len`: 列表长度
///
/// 返回:
/// - 不越界的新下标
fn next_index(index: usize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    (index + 1).min(len - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造供应商模型候选。
    ///
    /// 参数:
    /// - `provider_id`: 供应商标识
    /// - `provider_name`: 供应商展示名
    /// - `model`: 模型名称
    ///
    /// 返回:
    /// - 供应商模型候选
    fn choice(provider_id: &str, provider_name: &str, model: &str) -> ProviderModelChoice {
        ProviderModelChoice {
            provider_id: provider_id.to_string(),
            provider_name: provider_name.to_string(),
            model: model.to_string(),
        }
    }

    /// 构造含两个供应商模型与三档思考等级的状态。
    ///
    /// 返回:
    /// - 选择状态
    fn state() -> PickerState {
        PickerState::new(
            vec![
                choice("openai", "OpenAI", "gpt-5"),
                choice("deepseek", "DeepSeek", "deepseek-chat"),
                choice("openai", "OpenAI", "gpt-5-mini"),
            ],
            vec!["auto", "high", "max"],
            "openai",
            "gpt-5-mini",
            "high",
        )
    }

    /// 【CLI】【模型选择】验证初始下标定位到当前供应商和模型。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn starts_at_the_current_selection() {
        let picker = state();
        let selected = picker.selected_model().unwrap();

        assert_eq!(selected.provider_id, "openai");
        assert_eq!(selected.model, "gpt-5-mini");
        assert_eq!(picker.selected_level(), "high");
        assert_eq!(picker.column(), PickerColumn::Model);
    }

    /// 【CLI】【模型选择】验证上下键只影响当前聚焦列。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn vertical_moves_stay_within_the_focused_column() {
        let mut picker = state();

        picker.move_up();
        assert_eq!(picker.selected_model().unwrap().model, "deepseek-chat");
        assert_eq!(picker.selected_level(), "high", "思考列不应被模型移动影响");

        picker.focus_thinking();
        picker.move_down();
        assert_eq!(picker.selected_level(), "max");
        assert_eq!(
            picker.selected_model().unwrap().provider_id,
            "deepseek",
            "模型列应保持不变"
        );
    }

    /// 【CLI】【模型选择】验证移动在列表两端收敛而不回绕。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn moves_clamp_at_both_ends() {
        let mut picker = state();

        picker.move_up();
        picker.move_up();
        picker.move_up();
        assert_eq!(picker.model_index(), 0);

        picker.move_down();
        picker.move_down();
        picker.move_down();
        picker.move_down();
        assert_eq!(picker.model_index(), 2, "下移不应越过末项");
    }

    /// 【CLI】【模型选择】验证左右切换只改变焦点、不改变取值。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn horizontal_moves_only_change_focus() {
        let mut picker = state();
        let before = (picker.model_index(), picker.level_index());

        picker.focus_thinking();
        assert_eq!(picker.column(), PickerColumn::Thinking);
        picker.focus_model();
        assert_eq!(picker.column(), PickerColumn::Model);

        assert_eq!((picker.model_index(), picker.level_index()), before);
    }

    /// 【CLI】【模型选择】验证无可选模型时不会 panic。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn handles_an_empty_model_list() {
        let mut picker = PickerState::new(Vec::new(), vec!["auto"], "", "", "auto");

        picker.move_down();
        picker.move_up();

        assert!(picker.selected_model().is_none());
        assert_eq!(picker.selected_level(), "auto");
    }

    /// 【CLI】【模型选择】验证过滤同时匹配供应商与模型名称且忽略大小写。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn filters_models_by_provider_and_model_name() {
        let mut picker = state();

        for ch in "DEEP".chars() {
            picker.push_filter(ch);
        }
        assert_eq!(picker.models().len(), 1);
        assert_eq!(picker.selected_model().unwrap().provider_id, "deepseek");

        picker.clear_filter();
        for ch in "MINI".chars() {
            picker.push_filter(ch);
        }
        assert_eq!(picker.models().len(), 1);
        assert_eq!(picker.selected_model().unwrap().model, "gpt-5-mini");
        picker.clear_filter();
        assert_eq!(picker.models().len(), 3);
    }

    /// 【CLI】【模型选择】验证无匹配项时选择为空并可通过退格恢复。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn backspace_restores_filtered_models() {
        let mut picker = state();

        for ch in "missing".chars() {
            picker.push_filter(ch);
        }
        assert!(picker.selected_model().is_none());
        for _ in 0..7 {
            picker.pop_filter();
        }
        assert_eq!(picker.models().len(), 3);
    }

    /// 【CLI】【模型选择】验证模型窗口始终包含当前选中项。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn model_window_tracks_the_selection() {
        let models = (0..20)
            .map(|index| choice("provider", "Provider", &format!("model-{index}")))
            .collect();
        let mut picker = PickerState::new(models, vec!["auto"], "provider", "model-0", "auto");
        for _ in 0..15 {
            picker.move_down();
        }

        let (start, window) = picker.model_window(8);
        assert!(start <= picker.model_index());
        assert!(start + window.len() > picker.model_index());
        assert_eq!(window.len(), 8);
    }
}
