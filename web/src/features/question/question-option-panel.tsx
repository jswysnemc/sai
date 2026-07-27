import { Check } from "lucide-react";
import type { KeyboardEvent } from "react";
import type { QuestionPrompt } from "../../api/contracts";
import { Button } from "../../shared/ui/button/button";
import { TextArea } from "../../shared/ui/form/text-area";
import { useI18n } from "../i18n/use-i18n";

type QuestionOptionPanelProps = {
  question: QuestionPrompt;
  selected: string[];
  customDraft: string;
  interactive: boolean;
  onToggle: (label: string) => void;
  onCustomDraft: (value: string) => void;
  onSaveCustom: () => void;
};

/**
 * 渲染单个结构化问题、编号选项和可选自定义回答。
 *
 * @param props 问题内容、当前答案和交互回调
 * @returns 紧凑的问题选择面板
 */
export function QuestionOptionPanel({
  question,
  selected,
  customDraft,
  interactive,
  onToggle,
  onCustomDraft,
  onSaveCustom
}: QuestionOptionPanelProps) {
  const { t } = useI18n();
  const multiple = Boolean(question.multiple);
  const allowCustom = question.custom !== false;

  return (
    <div className="question-panel">
      <div className="question-text">{question.question}</div>
      <div className="question-options" role="group" aria-label={question.question}>
        {question.options.map((option, index) => {
          const answerValue = option.value ?? option.label;
          const active = selected.includes(answerValue);
          return (
            <Button
              key={answerValue}
              className={`question-option ${active ? "is-selected" : ""}`}
              disabled={!interactive}
              aria-pressed={active}
              onClick={() => onToggle(answerValue)}
              onKeyDown={(event) => moveOptionFocus(event, multiple)}
            >
              <span className="question-option-index" aria-hidden>{index + 1}.</span>
              <span className="question-option-copy">
                <strong>{option.label}</strong>
                {option.description && <span>{option.description}</span>}
              </span>
              <span className="question-option-selection" aria-hidden>
                {multiple && active ? <Check size={14} /> : null}
              </span>
            </Button>
          );
        })}
      </div>
      {allowCustom && interactive && (
        <label className="question-custom">
          <span>{t("Custom answer", "自定义回答")}</span>
          <TextArea value={customDraft} onChange={(event) => onCustomDraft(event.target.value)} placeholder={t("Enter another answer", "输入其他回答")} />
          <Button className="question-custom-save" disabled={!customDraft.trim()} onClick={onSaveCustom}>
            {t("Use this answer", "使用此回答")}
          </Button>
        </label>
      )}
    </div>
  );
}

/**
 * 使用上下方向键在同一问题的选项间循环移动焦点。
 *
 * 单选问题会同步选择焦点项；多选问题只移动焦点，避免意外切换已有答案。
 *
 * @param event 选项按钮键盘事件
 * @param multiple 当前问题是否允许多选
 * @returns 无返回值
 */
function moveOptionFocus(event: KeyboardEvent<HTMLButtonElement>, multiple: boolean): void {
  if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
  const container = event.currentTarget.parentElement;
  if (!container) return;
  const options = Array.from(container.querySelectorAll<HTMLButtonElement>(".question-option:not(:disabled)"));
  const currentIndex = options.indexOf(event.currentTarget);
  if (currentIndex < 0 || options.length < 2) return;

  // 1. 按方向循环计算目标选项
  event.preventDefault();
  const offset = event.key === "ArrowDown" ? 1 : -1;
  const nextIndex = (currentIndex + offset + options.length) % options.length;
  const nextOption = options[nextIndex];

  // 2. 单选同步答案，多选仅移动焦点
  nextOption.focus();
  if (!multiple) nextOption.click();
}
