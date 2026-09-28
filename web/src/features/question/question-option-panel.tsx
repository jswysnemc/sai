import type { KeyboardEvent, ReactNode } from "react";
import type { QuestionPrompt } from "../../api/contracts";
import { Button } from "../../shared/ui/button/button";
import { QuestionCustomAnswer } from "./question-custom-answer";
import { QuestionSelectionMark } from "./question-selection-mark";
import type { Translate } from "./question-card-state";

type QuestionOptionPanelProps = {
  question: QuestionPrompt;
  selected: string[];
  customDraft: string;
  attachmentKey?: string;
  interactive: boolean;
  onToggle: (value: string) => void;
  onCustomDraft: (value: string) => void;
  onSaveCustom: (images: string[]) => void;
  /** 多问题步进指示器，与问题文本首基线对齐放在同一行右侧 */
  steps?: ReactNode;
  t: Translate;
};

/**
 * 渲染当前待答问题、选项列表和折叠的自定义回答。
 *
 * @param props 问题内容、当前答案和交互回调
 * @returns 紧凑的问题选择面板
 */
export function QuestionOptionPanel({ question, selected, customDraft, attachmentKey = "question", interactive, onToggle, onCustomDraft, onSaveCustom, steps, t }: QuestionOptionPanelProps) {
  const multiple = Boolean(question.multiple);
  const allowCustom = question.custom !== false && interactive;
  // 1. 不在预设选项里的已选值来自自定义回答，单独列出便于确认
  const optionValues = new Set(question.options.map((option) => option.value ?? option.label));
  const customSelected = selected.filter((value) => !optionValues.has(value));

  return (
    <div className="question-panel">
      <div className="question-text-row">
        <p className="question-text">{question.question}</p>
        {steps}
      </div>
      <div className="question-options" role="group" aria-label={question.question}>
        {question.options.map((option) => {
          const value = option.value ?? option.label;
          const active = selected.includes(value);
          return (
            <Button
              key={value}
              variant="ghost"
              className={`question-option${active ? " is-selected" : ""}`}
              disabled={!interactive}
              aria-pressed={active}
              onClick={() => onToggle(value)}
              onKeyDown={moveOptionFocus}
            >
              <span className="question-option-line icon-label">
                <QuestionSelectionMark multiple={multiple} selected={active} />
                <span className="question-option-copy">
                  <strong>{option.label}</strong>
                  {option.description && <span>{option.description}</span>}
                </span>
              </span>
            </Button>
          );
        })}
        {customSelected.map((value) => (
          <div key={`custom-${value}`} className="question-option is-selected is-custom">
            <span className="question-option-line icon-label">
              <QuestionSelectionMark multiple={multiple} selected />
              <span className="question-option-copy"><strong>{value}</strong></span>
            </span>
          </div>
        ))}
      </div>
      {allowCustom && <QuestionCustomAnswer key={attachmentKey} attachmentKey={attachmentKey} allowImages={!question.validation} openInitially={question.options.length === 0} draft={customDraft} onDraft={onCustomDraft} onSave={onSaveCustom} t={t} />}
    </div>
  );
}

/**
 * 使用上下方向键在同一问题的选项间循环移动焦点。
 *
 * 方向键只移动焦点，选中交给回车/空格（触发原生 click），
 * 否则单选的「选中即推进」会让方向键浏览直接跳到下一题。
 *
 * @param event 选项按钮键盘事件
 * @returns 无返回值
 */
function moveOptionFocus(event: KeyboardEvent<HTMLButtonElement>): void {
  if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
  const container = event.currentTarget.parentElement;
  if (!container) return;
  const options = Array.from(container.querySelectorAll<HTMLButtonElement>("button.question-option:not(:disabled)"));
  const currentIndex = options.indexOf(event.currentTarget);
  if (currentIndex < 0 || options.length < 2) return;

  // 1. 按方向循环计算目标选项并移动焦点
  event.preventDefault();
  const offset = event.key === "ArrowDown" ? 1 : -1;
  options[(currentIndex + offset + options.length) % options.length].focus();
}
