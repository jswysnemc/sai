use super::text::display_inline;
use crate::i18n::text as t;
use crate::question::{QuestionAnswers, QuestionRequest};

/// 生成提问完成后的紧凑摘要行。
///
/// 参数:
/// - `request`: 原始问题集合
/// - `answers`: 用户提交的答案
/// - `max_lines`: 摘要最多使用的行数
///
/// 返回:
/// - 已限制行数的纯文本摘要
pub(super) fn answered_summary_lines(
    request: &QuestionRequest,
    answers: &QuestionAnswers,
    max_lines: usize,
) -> Vec<String> {
    if max_lines == 0 {
        return Vec::new();
    }
    let mut details = Vec::new();
    let mut shown = 0usize;
    for (question, selected) in request.questions.iter().zip(answers) {
        let remaining = max_lines.saturating_sub(1 + details.len());
        if remaining == 0 {
            break;
        }
        let question_text = display_inline(&question.question);
        let answer_text = display_inline(&selected.join(t(" / ", "、")));
        if remaining >= 2 {
            details.push(format!("{} {}", t("Question:", "问题："), question_text));
            details.push(format!("{} {}", t("Answer:", "回答："), answer_text));
        } else {
            details.push(format!(
                "{} {}  {} {}",
                t("Question:", "问题："),
                question_text,
                t("Answer:", "回答："),
                answer_text
            ));
        }
        shown += 1;
    }
    let omitted = request.questions.len().saturating_sub(shown);
    let heading = if omitted == 0 {
        format!(
            "{} {} {}",
            t("Answered", "已回答"),
            request.questions.len(),
            t("questions", "个问题")
        )
    } else {
        format!(
            "{} {} {}  {} {}",
            t("Answered", "已回答"),
            request.questions.len(),
            t("questions", "个问题"),
            t("omitted", "省略"),
            omitted
        )
    };
    let mut lines = Vec::with_capacity(details.len() + 1);
    lines.push(heading);
    lines.extend(details);
    lines
}

/// 【终端提问】【完成卡片】参数为问题、答案与可用尺寸，返回无背景色的紧凑回答卡片。
pub(super) fn answered_card_lines(
    request: &QuestionRequest,
    answers: &QuestionAnswers,
    cols: usize,
    max_lines: usize,
) -> Vec<String> {
    let card = super::card::CardLayout::new(cols, max_lines);
    let summary = answered_summary_lines(request, answers, max_lines);
    let Some(heading) = summary.first() else {
        return Vec::new();
    };
    let mut lines = vec![if card.framed {
        card.heading(heading)
    } else {
        card.row(heading)
    }];
    let remaining = max_lines.saturating_sub(lines.len() + usize::from(card.framed));
    lines.extend(
        summary
            .iter()
            .skip(1)
            .flat_map(|line| super::text::wrap_display_text(line, card.content_width()))
            .take(remaining)
            .map(|line| card.row(&line)),
    );
    if card.framed {
        lines.push(card.rule(true));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::question::{QuestionOption, QuestionPrompt};

    /// 验证完成摘要直接展示问题正文与对应答案。
    ///
    /// 参数:
    /// - 无
    ///
    /// 返回:
    /// - 无
    #[test]
    fn answered_summary_shows_question_and_answer() {
        let request = QuestionRequest {
            questions: vec![QuestionPrompt {
                header: "继续前确认".to_string(),
                question: "是否继续构建复现？".to_string(),
                options: vec![QuestionOption {
                    label: "继续".to_string(),
                    description: String::new(),
                    value: Some("继续构建复现".to_string()),
                }],
                multiple: false,
                custom: false,
                required: true,
                default_answers: Vec::new(),
                validation: None,
            }],
        };
        let answers = vec![vec!["继续构建复现".to_string()]];

        let lines = answered_summary_lines(&request, &answers, 3);
        let joined = lines.join("\n");

        assert!(joined.contains("是否继续构建复现？"));
        assert!(joined.contains("继续构建复现"));
    }
}
