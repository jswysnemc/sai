/// 选择器当前聚焦的列。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PickerColumn {
    /// 模型列
    Model,
    /// 思考等级列
    Thinking,
}

/// models 命令的交互选择状态。
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PickerState {
    /// 可选模型，空列表表示当前供应商未配置模型
    models: Vec<String>,
    /// 可选思考等级
    levels: Vec<&'static str>,
    model_index: usize,
    level_index: usize,
    column: PickerColumn,
}

impl PickerState {
    /// 创建选择状态，并定位到当前生效的模型与思考等级。
    ///
    /// 参数:
    /// - `models`: 当前供应商可用模型
    /// - `levels`: 可选思考等级
    /// - `current_model`: 当前生效模型
    /// - `current_level`: 当前生效思考等级
    ///
    /// 返回:
    /// - 已定位到当前取值的选择状态
    pub(super) fn new(
        models: Vec<String>,
        levels: Vec<&'static str>,
        current_model: &str,
        current_level: &str,
    ) -> Self {
        let model_index = models
            .iter()
            .position(|model| model == current_model)
            .unwrap_or(0);
        let level_index = levels
            .iter()
            .position(|level| *level == current_level)
            .unwrap_or(0);
        Self {
            models,
            levels,
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

    /// 返回当前聚焦的列。
    ///
    /// 返回:
    /// - 聚焦列
    pub(super) fn column(&self) -> PickerColumn {
        self.column
    }

    /// 返回可选模型列表。
    ///
    /// 返回:
    /// - 模型列表
    pub(super) fn models(&self) -> &[String] {
        &self.models
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

    /// 返回当前选中的模型。
    ///
    /// 返回:
    /// - 模型名称；无可选模型时为 None
    pub(super) fn selected_model(&self) -> Option<&str> {
        self.models.get(self.model_index).map(String::as_str)
    }

    /// 返回当前选中的思考等级。
    ///
    /// 返回:
    /// - 思考等级
    pub(super) fn selected_level(&self) -> &'static str {
        self.levels.get(self.level_index).copied().unwrap_or("auto")
    }
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

    /// 构造含两个模型与三档思考等级的状态。
    ///
    /// 返回:
    /// - 选择状态
    fn state() -> PickerState {
        PickerState::new(
            vec!["gpt-5".to_string(), "gpt-5-mini".to_string()],
            vec!["auto", "high", "max"],
            "gpt-5-mini",
            "high",
        )
    }

    /// 【CLI】【模型选择】验证初始下标定位到当前生效取值。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn starts_at_the_current_selection() {
        let picker = state();

        assert_eq!(picker.selected_model(), Some("gpt-5-mini"));
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
        assert_eq!(picker.selected_model(), Some("gpt-5"));
        assert_eq!(picker.selected_level(), "high", "思考列不应被模型移动影响");

        picker.focus_thinking();
        picker.move_down();
        assert_eq!(picker.selected_level(), "max");
        assert_eq!(picker.selected_model(), Some("gpt-5"), "模型列应保持不变");
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
        assert_eq!(picker.model_index(), 0);

        picker.move_down();
        picker.move_down();
        picker.move_down();
        assert_eq!(picker.model_index(), 1, "下移不应越过末项");
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
        let mut picker = PickerState::new(Vec::new(), vec!["auto"], "", "auto");

        picker.move_down();
        picker.move_up();

        assert_eq!(picker.selected_model(), None);
        assert_eq!(picker.selected_level(), "auto");
    }
}
