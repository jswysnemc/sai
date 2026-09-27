import type { QuestionPrompt } from "../../api/contracts";
import type { Translate } from "./question-card-state";

type QuestionHistoryProps = {
  questions: QuestionPrompt[];
  summary: string[];
  t: Translate;
};

/**
 * 渲染已处理提问的历史选择：每个问题一行「标题 回答」。
 *
 * @param props 问题集合、每题回答摘要与文案函数
 * @returns 紧凑的历史选择列表
 */
export function QuestionHistory({ questions, summary, t }: QuestionHistoryProps) {
  return (
    <dl className="question-history">
      {questions.map((question, index) => (
        <div key={`${question.header}-${index}`} className="question-history-row">
          <dt title={question.question}>{question.header}</dt>
          <dd>{summary[index] || (question.required === false ? t("Skipped", "已跳过") : t("Unanswered", "未回答"))}</dd>
        </div>
      ))}
    </dl>
  );
}
