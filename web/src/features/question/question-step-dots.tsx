import type { KeyboardEvent } from "react";
import { Button } from "../../shared/ui/button/button";
import type { Translate } from "./question-card-state";

type QuestionStepDotsProps = {
  count: number;
  current: number;
  /** 每个问题是否已满足作答要求 */
  satisfied: boolean[];
  onSelect: (index: number) => void;
  t: Translate;
};

/**
 * 渲染多问题的点状步进指示器，点击或左右方向键切换问题。
 *
 * @param props 问题数量、当前位置、完成情况与切换回调
 * @returns 紧凑的步进指示器；单个问题时不渲染
 */
export function QuestionStepDots({ count, current, satisfied, onSelect, t }: QuestionStepDotsProps) {
  if (count < 2) return null;

  /**
   * 左右方向键在步进点之间移动。
   *
   * @param event 键盘事件
   * @returns 无返回值
   */
  const handleKeyDown = (event: KeyboardEvent<HTMLDivElement>): void => {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    event.preventDefault();
    const offset = event.key === "ArrowRight" ? 1 : -1;
    onSelect((current + offset + count) % count);
  };

  return (
    <div className="question-steps" role="tablist" aria-label={t("Questions", "问题")} onKeyDown={handleKeyDown}>
      {Array.from({ length: count }, (_, index) => {
        const state = index === current ? "is-current" : satisfied[index] ? "is-done" : "";
        return (
          <Button
            key={index}
            variant="ghost"
            size="icon"
            role="tab"
            className={`question-step ${state}`.trim()}
            aria-selected={index === current}
            aria-label={t(`Question ${index + 1} of ${count}`, `第 ${index + 1} 个问题，共 ${count} 个`)}
            tabIndex={index === current ? 0 : -1}
            onClick={() => onSelect(index)}
          >
            <span className="question-step-dot" aria-hidden />
          </Button>
        );
      })}
      <span className="question-steps-position" aria-hidden>{current + 1}/{count}</span>
    </div>
  );
}
