use super::*;

pub(super) struct QuestionState {
    pub(super) tab: usize,
    pub(super) selected: Vec<usize>,
    pub(super) scroll_starts: Vec<usize>,
    pub(super) answers: QuestionAnswers,
    pub(super) custom_answers: Vec<String>,
    pub(super) editing: bool,
    pub(super) edit_buffer: String,
    pub(super) edit_cursor: usize,
    pub(super) cancel_armed_until: Option<Instant>,
}

impl QuestionState {
    /// 根据提问请求创建初始交互状态。
    ///
    /// 参数:
    /// - `request`: 结构化提问请求
    ///
    /// 返回:
    /// - 尚未回答的初始状态
    pub(super) fn new(request: &QuestionRequest) -> Self {
        Self {
            tab: 0,
            selected: request
                .questions
                .iter()
                .map(|question| {
                    question
                        .default_answers
                        .first()
                        .and_then(|answer| {
                            question
                                .options
                                .iter()
                                .position(|option| option.answer_value() == answer)
                                .or_else(|| question.custom.then_some(question.options.len()))
                        })
                        .unwrap_or(0)
                })
                .collect(),
            scroll_starts: vec![0; request.questions.len() + usize::from(request.needs_review())],
            answers: request
                .questions
                .iter()
                .map(|question| question.default_answers.clone())
                .collect(),
            custom_answers: request
                .questions
                .iter()
                .map(default_custom_answer)
                .collect(),
            editing: false,
            edit_buffer: String::new(),
            edit_cursor: 0,
            cancel_armed_until: None,
        }
    }

    /// 判断当前标签是否为最终确认页。
    ///
    /// 参数:
    /// - `request`: 结构化提问请求
    ///
    /// 返回:
    /// - 当前位于确认页时返回 `true`
    pub(super) fn on_confirm(&self, request: &QuestionRequest) -> bool {
        request.needs_review() && self.tab == request.questions.len()
    }

    /// 计算问题标签和确认标签的总数。
    ///
    /// 参数:
    /// - `request`: 结构化提问请求
    ///
    /// 返回:
    /// - 可切换标签总数
    pub(super) fn tab_count(&self, request: &QuestionRequest) -> usize {
        request.questions.len() + usize::from(request.needs_review())
    }

    /// 切换到前一个问题或确认标签。
    ///
    /// 参数:
    /// - `request`: 结构化提问请求
    ///
    /// 返回:
    /// - 无
    pub(super) fn previous_tab(&mut self, request: &QuestionRequest) {
        let count = self.tab_count(request);
        self.tab = (self.tab + count - 1) % count;
    }

    /// 切换到后一个问题或确认标签。
    ///
    /// 参数:
    /// - `request`: 结构化提问请求
    ///
    /// 返回:
    /// - 无
    pub(super) fn next_tab(&mut self, request: &QuestionRequest) {
        self.tab = (self.tab + 1) % self.tab_count(request);
    }

    /// 在当前问题中选择前一个选项。
    ///
    /// 参数:
    /// - `question`: 当前问题
    ///
    /// 返回:
    /// - 无
    pub(super) fn previous_option(&mut self, question: &QuestionPrompt) {
        let count = option_count(question);
        if count > 0 {
            let selected = &mut self.selected[self.tab];
            *selected = (*selected + count - 1) % count;
        }
    }

    /// 在当前问题中选择后一个选项。
    ///
    /// 参数:
    /// - `question`: 当前问题
    ///
    /// 返回:
    /// - 无
    pub(super) fn next_option(&mut self, question: &QuestionPrompt) {
        let count = option_count(question);
        if count > 0 {
            self.selected[self.tab] = (self.selected[self.tab] + 1) % count;
        }
    }

    /// 激活当前选项，或进入自定义答案编辑状态。
    ///
    /// 参数:
    /// - `request`: 结构化提问请求
    ///
    /// 返回:
    /// - 操作成功时返回空结果
    pub(super) fn activate_current(&mut self, request: &QuestionRequest) -> Result<()> {
        let question = &request.questions[self.tab];
        let selected = self.selected[self.tab];
        if selected == question.options.len() && question.custom {
            self.editing = true;
            self.edit_buffer = self.custom_answers[self.tab].clone();
            self.edit_cursor = self.edit_buffer.chars().count();
            return Ok(());
        }
        let Some(option) = question.options.get(selected) else {
            bail!(t(
                "selected question option is out of range",
                "选中的问题选项超出范围"
            ));
        };
        if question.multiple {
            toggle_answer(&mut self.answers[self.tab], option.answer_value());
        } else {
            self.answers[self.tab] = vec![option.answer_value().to_string()];
            self.advance_after_single(request);
        }
        Ok(())
    }

    /// 【终端提问】【编号选择】参数为问题集合与从一开始的编号；有效编号执行选择或编辑，返回结果。
    pub(super) fn activate_number(
        &mut self,
        request: &QuestionRequest,
        number: usize,
    ) -> Result<()> {
        let question = &request.questions[self.tab];
        if number == 0 || number > option_count(question) {
            return Ok(());
        }
        self.selected[self.tab] = number - 1;
        if question.multiple {
            self.toggle_current(request)
        } else {
            self.activate_current(request)
        }
    }

    /// 【终端提问】【多选继续】参数为问题集合，返回推进结果；自定义选项进入编辑，不隐式勾选。
    pub(super) fn continue_multiple(&mut self, request: &QuestionRequest) -> Result<()> {
        let question = &request.questions[self.tab];
        if question.custom && self.selected[self.tab] == question.options.len() {
            self.activate_current(request)
        } else {
            self.next_tab(request);
            Ok(())
        }
    }

    /// 切换当前多选答案的选中状态。
    ///
    /// 参数:
    /// - `request`: 结构化提问请求
    ///
    /// 返回:
    /// - 操作成功时返回空结果
    pub(super) fn toggle_current(&mut self, request: &QuestionRequest) -> Result<()> {
        let question = &request.questions[self.tab];
        let selected = self.selected[self.tab];
        if selected == question.options.len() && question.custom {
            let custom = self.custom_answers[self.tab].trim();
            if custom.is_empty() {
                return self.activate_current(request);
            }
            toggle_answer(&mut self.answers[self.tab], custom);
            return Ok(());
        }
        self.activate_current(request)
    }

    /// 单选完成后推进到下一个标签。
    ///
    /// 参数:
    /// - `request`: 结构化提问请求
    ///
    /// 返回:
    /// - 无
    pub(super) fn advance_after_single(&mut self, request: &QuestionRequest) {
        if request.questions.len() == 1 && !request.needs_review() {
            return;
        }
        self.tab = (self.tab + 1).min(self.tab_count(request) - 1);
    }

    /// 将焦点移动到第一个未回答问题。
    ///
    /// 参数:
    /// - `request`: 结构化提问请求
    ///
    /// 返回:
    /// - 无
    pub(super) fn go_to_first_unanswered(&mut self, request: &QuestionRequest) {
        if let Some(index) = self
            .answers
            .iter()
            .enumerate()
            .position(|(index, answers)| answers.is_empty() && request.questions[index].required)
        {
            self.tab = index.min(request.questions.len().saturating_sub(1));
        }
    }
}

/// 返回不属于预设选项的默认自定义答案。
///
/// 参数:
/// - `question`: 当前结构化问题
///
/// 返回:
/// - 可放入自定义编辑器的默认答案
pub(super) fn default_custom_answer(question: &QuestionPrompt) -> String {
    question
        .default_answers
        .iter()
        .find(|answer| {
            !question
                .options
                .iter()
                .any(|option| option.answer_value() == answer.as_str())
        })
        .cloned()
        .unwrap_or_default()
}

/// 【终端提问】【答案编辑】参数为问题、状态与按键，返回是否保存了自定义答案。
pub(super) fn handle_editing_key(
    request: &QuestionRequest,
    state: &mut QuestionState,
    key: KeyEvent,
) -> Result<bool> {
    match key.code {
        KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            insert_text(&mut state.edit_buffer, &mut state.edit_cursor, "\n");
        }
        KeyCode::Esc => {
            state.editing = false;
            state.edit_buffer.clear();
            state.edit_cursor = 0;
        }
        KeyCode::Enter => {
            let value = state.edit_buffer.trim().to_string();
            if value.is_empty() {
                let previous = std::mem::take(&mut state.custom_answers[state.tab]);
                state.answers[state.tab].retain(|answer| answer != &previous);
                state.editing = false;
                state.edit_buffer.clear();
                state.edit_cursor = 0;
                return Ok(false);
            }
            let question = &request.questions[state.tab];
            let previous = std::mem::replace(&mut state.custom_answers[state.tab], value.clone());
            if !previous.is_empty() {
                state.answers[state.tab].retain(|answer| answer != &previous);
            }
            if question.multiple {
                if !state.answers[state.tab].contains(&value) {
                    state.answers[state.tab].push(value);
                }
            } else {
                state.answers[state.tab] = vec![value];
            }
            state.editing = false;
            state.edit_buffer.clear();
            state.edit_cursor = 0;
            if !question.multiple {
                state.advance_after_single(request);
            }
            return Ok(true);
        }
        KeyCode::Left => state.edit_cursor = state.edit_cursor.saturating_sub(1),
        KeyCode::Right => {
            state.edit_cursor = (state.edit_cursor + 1).min(state.edit_buffer.chars().count())
        }
        KeyCode::Home => state.edit_cursor = 0,
        KeyCode::End => state.edit_cursor = state.edit_buffer.chars().count(),
        KeyCode::Backspace => remove_before_cursor(&mut state.edit_buffer, &mut state.edit_cursor),
        KeyCode::Delete => remove_at_cursor(&mut state.edit_buffer, state.edit_cursor),
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            insert_text(
                &mut state.edit_buffer,
                &mut state.edit_cursor,
                &ch.to_string(),
            );
        }
        _ => {}
    }
    Ok(false)
}

/// 校验当前回答是否达到提交条件。
///
/// 参数:
/// - `request`: 结构化提问请求
/// - `state`: 当前回答状态
///
/// 返回:
/// - 可以提交时返回答案，否则返回空
pub(super) fn submitted_answers(
    request: &QuestionRequest,
    state: &QuestionState,
) -> Result<Option<QuestionAnswers>> {
    if state.editing
        || state
            .answers
            .iter()
            .enumerate()
            .any(|(index, answers)| answers.is_empty() && request.questions[index].required)
    {
        return Ok(None);
    }
    if request.needs_review() && !state.on_confirm(request) {
        return Ok(None);
    }
    validate_answers(request, &state.answers)?;
    Ok(Some(state.answers.clone()))
}

/// 计算问题的预设选项和自定义选项总数。
///
/// 参数:
/// - `question`: 当前问题
///
/// 返回:
/// - 可选择项总数
fn option_count(question: &QuestionPrompt) -> usize {
    question.options.len() + usize::from(question.custom)
}

/// 切换指定答案在多选结果中的存在状态。
///
/// 参数:
/// - `answers`: 当前多选答案
/// - `value`: 需要切换的答案
///
/// 返回:
/// - 无
fn toggle_answer(answers: &mut Vec<String>, value: &str) {
    if let Some(index) = answers.iter().position(|answer| answer == value) {
        answers.remove(index);
    } else {
        answers.push(value.to_string());
    }
}
