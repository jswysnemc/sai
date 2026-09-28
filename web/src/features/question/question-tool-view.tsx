import { Image } from "../../shared/ui/icons";
import { useI18n } from "../i18n/use-i18n";
import { ToolPanel } from "../chat/tool-renderers/layout/tool-panel";
import { QuestionSelectionMark } from "./question-selection-mark";
import type { parseQuestionTool } from "./question-tool-data";
import "./question-request-card.css";
import "./question-tool-view.css";

/**
 * 以题目、选项和实际回答展示提问调用，历史查看不包含可提交按钮。
 * @param data 已校验的提问工具数据
 * @returns 紧凑的结构化问答视图
 */
export function QuestionToolView({ data }: { data: NonNullable<ReturnType<typeof parseQuestionTool>> }) {
  const { t } = useI18n();
  return <ToolPanel className="question-tool-view">
    {data.questions.map(({ question, selected, imageCount }, index) => <section className="question-tool-item" key={index}>
      <header><strong>{question.header || t(`Question ${index + 1}`, `问题 ${index + 1}`)}</strong><span>{question.multiple ? t("Multiple choices", "多选") : t("Single answer", "单项回答")}</span></header>
      <p className="question-text">{question.question}</p>
      {question.options.length > 0 && <ul className="question-tool-options">{question.options.map((option) => <li key={option.value ?? option.label} data-selected={selected.includes(option.value ?? option.label) || undefined}>
        <QuestionSelectionMark multiple={Boolean(question.multiple)} selected={selected.includes(option.value ?? option.label)} />
        <div><strong>{option.label}</strong>{option.description && <p>{option.description}</p>}</div>
      </li>)}</ul>}
      <div className="question-tool-answer"><span>{t("Answer", "回答")}</span><p>{selected.length ? selected.join("\n") : data.status === "unavailable" ? t("Not answered", "未回答") : t("Waiting for an answer", "等待回答")}</p>
        {imageCount > 0 && <span className="icon-label"><Image size={12} />{t(`${imageCount} images attached`, `已附 ${imageCount} 张图片`)}</span>}
      </div>
    </section>)}
    {data.reason && <p className="question-request-error">{data.reason}</p>}
  </ToolPanel>;
}
