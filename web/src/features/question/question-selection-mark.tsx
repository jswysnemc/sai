import { CheckSquare2, Circle, CircleDot, Square } from "../../shared/ui/icons";

type QuestionSelectionMarkProps = {
  multiple: boolean;
  selected: boolean;
};

/**
 * 用语义化 SVG 渲染单选圈或复选框，替代原生控件外框。
 *
 * @param props 是否多选、是否已选中
 * @returns 14px 选择指示图标
 */
export function QuestionSelectionMark({ multiple, selected }: QuestionSelectionMarkProps) {
  if (multiple) return selected ? <CheckSquare2 size={14} aria-hidden /> : <Square size={14} aria-hidden />;
  return selected ? <CircleDot size={14} aria-hidden /> : <Circle size={14} aria-hidden />;
}
