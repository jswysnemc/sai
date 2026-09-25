import type { EditorView } from "@codemirror/view";
import { useLayoutEffect, useRef } from "react";
import { createPortal } from "react-dom";
import type { FormatAction } from "../editor-format-actions";
import { FormatToolbar } from "./format-toolbar";
import type { BubblePosition } from "./use-selection-bubble";
import "./editor-menus.css";

type SelectionBubbleProps = {
  view: EditorView;
  position: BubblePosition;
  groups: FormatAction[][];
  label: string;
};

/** 视口边缘留白（像素）。 */
const EDGE = 8;

/**
 * 选中文字时浮现的格式气泡。
 *
 * 以选区为锚点水平居中，贴近视口边缘时向内收，不遮挡选中的文字。
 *
 * @param props 编辑器视图、位置、动作分组与无障碍标签
 * @returns 固定定位的气泡
 */
export function SelectionBubble({ view, position, groups, label }: SelectionBubbleProps) {
  const ref = useRef<HTMLDivElement>(null);

  useLayoutEffect(() => {
    const element = ref.current;
    if (!element) return;
    const { width, height } = element.getBoundingClientRect();
    const left = Math.min(Math.max(EDGE, position.x - width / 2), window.innerWidth - width - EDGE);
    const top = position.placement === "above" ? position.y - height : position.y;
    element.style.left = `${left}px`;
    element.style.top = `${Math.max(EDGE, top)}px`;
  }, [position]);

  return createPortal(
    <div
      ref={ref}
      className="md-menu-panel md-selection-bubble"
      role="dialog"
      aria-label={label}
      style={{ position: "fixed", left: position.x, top: position.y }}
      onMouseDown={(event) => event.preventDefault()}
    >
      <FormatToolbar view={view} groups={groups} />
    </div>,
    document.body
  );
}
